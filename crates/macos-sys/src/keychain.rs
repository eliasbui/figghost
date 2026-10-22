//! Generic passwords of the login keychain, through the Security framework, so that a secret never passes
//! through the command line of the `security` tool. Reading an item another program made makes macOS ask the
//! user first; their refusal comes back as an error status like any other.

use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    data::{CFData, CFDataRef},
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
};

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    static kSecClass: CFStringRef;
    static kSecClassGenericPassword: CFStringRef;
    static kSecAttrService: CFStringRef;
    static kSecAttrAccount: CFStringRef;
    static kSecReturnData: CFStringRef;
    static kSecMatchLimit: CFStringRef;
    static kSecMatchLimitOne: CFStringRef;
    static kSecValueData: CFStringRef;
    fn SecItemCopyMatching(query: CFDictionaryRef, result: *mut CFTypeRef) -> i32;
    fn SecItemAdd(attributes: CFDictionaryRef, result: *mut CFTypeRef) -> i32;
    fn SecItemUpdate(query: CFDictionaryRef, changes: CFDictionaryRef) -> i32;
    fn SecItemDelete(query: CFDictionaryRef) -> i32;
}

/// `errSecItemNotFound`.
const NOT_FOUND: i32 = -25300;
/// `errSecUserCanceled` and `errSecAuthFailed`: the user said no, or did not give the password.
pub const REFUSED: [i32; 2] = [-128, -25293];

fn key(name: CFStringRef) -> CFString {
    // SAFETY: the Security framework's constants are valid CFStrings for the life of the process.
    unsafe { CFString::wrap_under_get_rule(name) }
}

/// The pairs that name one generic password, plus `extra`.
fn query(
    service: &str,
    account: &str,
    extra: &[(CFStringRef, CFType)],
) -> CFDictionary<CFString, CFType> {
    // SAFETY: as in `key`.
    let class = unsafe { key(kSecClass) };
    let mut pairs = vec![
        (class, key(unsafe { kSecClassGenericPassword }).as_CFType()),
        (
            key(unsafe { kSecAttrService }),
            CFString::new(service).as_CFType(),
        ),
        (
            key(unsafe { kSecAttrAccount }),
            CFString::new(account).as_CFType(),
        ),
    ];
    pairs.extend(
        extra
            .iter()
            .map(|(name, value)| (key(*name), value.clone())),
    );
    CFDictionary::from_CFType_pairs(&pairs)
}

/// The data of the item, `None` when there is none, or the status the framework refused with.
pub fn read(service: &str, account: &str) -> Result<Option<Vec<u8>>, i32> {
    // SAFETY: as in `key`.
    let (data, limit, one) = unsafe { (kSecReturnData, kSecMatchLimit, kSecMatchLimitOne) };
    let query = query(
        service,
        account,
        &[
            (data, CFBoolean::true_value().as_CFType()),
            (limit, key(one).as_CFType()),
        ],
    );
    let mut result: CFTypeRef = std::ptr::null();
    // SAFETY: the query is a valid dictionary and `result` a valid out value, owned by us when filled.
    let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) };
    match status {
        0 if !result.is_null() => {
            // SAFETY: with kSecReturnData and one match the result is a CFData we own.
            let data = unsafe { CFData::wrap_under_create_rule(result as CFDataRef) };
            Ok(Some(data.bytes().to_vec()))
        }
        0 | NOT_FOUND => Ok(None),
        status => Err(status),
    }
}

/// Replaces the data of the item, or adds the item when there is none. The access rules of an existing item
/// stay as they are, so the program that made it can still read it.
pub fn write(service: &str, account: &str, bytes: &[u8]) -> Result<(), i32> {
    // SAFETY: as in `key`.
    let value = unsafe { kSecValueData };
    let data = CFData::from_buffer(bytes).as_CFType();
    let changes = CFDictionary::from_CFType_pairs(&[(key(value), data.clone())]);
    let found = query(service, account, &[]);
    // SAFETY: both dictionaries are valid for the call.
    let status =
        unsafe { SecItemUpdate(found.as_concrete_TypeRef(), changes.as_concrete_TypeRef()) };
    if status != NOT_FOUND {
        return if status == 0 { Ok(()) } else { Err(status) };
    }
    let item = query(service, account, &[(value, data)]);
    // SAFETY: a valid dictionary; no result is asked for.
    match unsafe { SecItemAdd(item.as_concrete_TypeRef(), std::ptr::null_mut()) } {
        0 => Ok(()),
        status => Err(status),
    }
}

/// Removes the item; one that is not there is not an error.
pub fn delete(service: &str, account: &str) -> Result<(), i32> {
    let found = query(service, account, &[]);
    // SAFETY: a valid dictionary for the call.
    match unsafe { SecItemDelete(found.as_concrete_TypeRef()) } {
        0 | NOT_FOUND => Ok(()),
        status => Err(status),
    }
}
