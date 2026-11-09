// Words this shell has and the page of the current app does not: they are added to the extracted dictionaries.
export const shell = {
  en: {
    'fault.unsupported': 'That is not available in this version yet.',
    'fault.action_failed': 'That did not work.',
    'ui.ready': 'Ready',
    'ui.found': 'Found',
    'ui.missing': 'Not installed',
    'ui.problem': 'Problem',
    'ui.notes': '{count} notes',
    'ui.info': 'More about this',
  },
  vi: {
    'fault.unsupported': 'Chức năng này chưa có trong phiên bản này.',
    'fault.action_failed': 'Không thực hiện được.',
    'ui.ready': 'Sẵn sàng',
    'ui.found': 'Đã có',
    'ui.missing': 'Chưa cài',
    'ui.problem': 'Có vấn đề',
    'ui.notes': '{count} ghi chú',
    'ui.info': 'Xem thêm',
  },
} satisfies Record<'en' | 'vi', Record<string, string>>;
