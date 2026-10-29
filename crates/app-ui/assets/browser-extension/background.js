// Figghost Companion: talks to the Figghost app on this Mac through native messaging, and to nothing else.
// While Figghost says a Pomodoro work stage runs, the sites it names are blocked; the button and the menu on a page or a
// link send its address to Figghost's video download. It reads no page and keeps nothing. Without the app, nothing is
// blocked, and it looks for the app again every half minute.

const HOST = 'vn.figghost.browser';
const RULE = 1;
let port = null;

function connect() {
  if (port) return;
  try {
    port = chrome.runtime.connectNative(HOST);
  } catch (e) {
    port = null;
    return;
  }
  port.onMessage.addListener(onMessage);
  port.onDisconnect.addListener(() => {
    // Reading lastError tells Chrome it was seen: a missing host is expected until the bridge is installed.
    void chrome.runtime.lastError;
    port = null;
    block([]);
  });
  port.postMessage({ type: 'hello' });
}

function onMessage(message) {
  if (!message || typeof message !== 'object') return;
  if (message.type === 'state') {
    const sites = Array.isArray(message.sites) ? message.sites.filter((s) => typeof s === 'string') : [];
    block(message.block === true ? sites : []);
  } else if (message.type === 'download') {
    badge(message.ok === true ? 'OK' : '!', message.ok === true ? '#2e7d32' : '#c62828');
  }
}

// One rule that blocks the pages and frames of every site named, and its subdomains; no sites, no rule.
async function block(sites) {
  const addRules = sites.length
    ? [{
        id: RULE,
        priority: 1,
        action: { type: 'block' },
        condition: { requestDomains: sites.slice(0, 500), resourceTypes: ['main_frame', 'sub_frame'] },
      }]
    : [];
  try {
    await chrome.declarativeNetRequest.updateDynamicRules({ removeRuleIds: [RULE], addRules });
  } catch (e) {
    console.warn('figghost: the rule was not taken', e);
  }
}

function send(url) {
  if (!/^https?:\/\//i.test(url || '')) {
    badge('!', '#c62828');
    return;
  }
  if (!port) {
    connect();
  }
  if (!port) {
    badge('!', '#c62828');
    return;
  }
  port.postMessage({ type: 'download', url });
}

function badge(text, color) {
  chrome.action.setBadgeBackgroundColor({ color });
  chrome.action.setBadgeText({ text });
  setTimeout(() => chrome.action.setBadgeText({ text: '' }), 3000);
}

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({ id: 'figghost-link', title: chrome.i18n.getMessage('downloadLink'), contexts: ['link'] });
    chrome.contextMenus.create({ id: 'figghost-page', title: chrome.i18n.getMessage('downloadPage'), contexts: ['page', 'video'] });
  });
});
chrome.contextMenus.onClicked.addListener((info) => send(info.menuItemId === 'figghost-link' ? info.linkUrl : info.pageUrl));
chrome.action.onClicked.addListener((tab) => send(tab && tab.url));
chrome.alarms.create('figghost-look', { periodInMinutes: 0.5 });
chrome.alarms.onAlarm.addListener(connect);
chrome.runtime.onStartup.addListener(connect);
connect();
