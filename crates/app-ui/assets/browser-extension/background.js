// Figghost Companion: talks to the Figghost app on this Mac through native messaging, and to nothing else.
// While Figghost says a Pomodoro work stage runs, the sites it names are blocked; the button and the menu on a page or a
// link send its address to Figghost's video download. It reads no page and keeps nothing. Without the app, nothing is
// blocked, and it looks for the app again every half minute. When the person chooses a playback speed in Figghost and
// allows it (the menu of the button asks once for access to pages), the videos that play are set to that speed.

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
    useSpeed(typeof message.speed === 'number' && message.speed >= 0.25 && message.speed <= 4 ? message.speed : null);
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

// The speed Figghost asks for, or null to leave the videos alone.
let speed = null;

function useSpeed(rate) {
  speed = rate;
  if (rate !== null) setSpeed(rate);
}

async function allowed() {
  try {
    return await chrome.permissions.contains({ origins: ['<all_urls>'] });
  } catch (e) {
    return false;
  }
}

// Sets the speed of the media that plays in the tabs that make a sound; the rest are left as they are.
async function setSpeed(rate, only) {
  if (!(await allowed())) {
    badge('?', '#ef6c00');
    return;
  }
  const tabs = only !== undefined ? [{ id: only }] : await chrome.tabs.query({ audible: true });
  for (const tab of tabs) {
    chrome.scripting
      .executeScript({
        target: { tabId: tab.id, allFrames: true },
        args: [rate],
        func: (r) => {
          for (const media of document.querySelectorAll('video, audio')) {
            if (!media.paused) media.playbackRate = r;
          }
        },
      })
      .catch(() => {});
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
    chrome.contextMenus.create({ id: 'figghost-speed', title: chrome.i18n.getMessage('allowSpeed'), contexts: ['action'] });
  });
});
chrome.contextMenus.onClicked.addListener((info) => {
  if (info.menuItemId === 'figghost-speed') {
    // A click on a menu is the gesture that lets the browser ask for access to pages.
    chrome.permissions.request({ origins: ['<all_urls>'] }).then((granted) => {
      if (granted && speed !== null) setSpeed(speed);
    });
    return;
  }
  send(info.menuItemId === 'figghost-link' ? info.linkUrl : info.pageUrl);
});
// A tab that starts to make a sound gets the speed too.
chrome.tabs.onUpdated.addListener((tabId, change) => {
  if (change.audible === true && speed !== null) setSpeed(speed, tabId);
});
chrome.action.onClicked.addListener((tab) => send(tab && tab.url));
chrome.alarms.create('figghost-look', { periodInMinutes: 0.5 });
chrome.alarms.onAlarm.addListener(() => {
  connect();
  if (speed !== null) setSpeed(speed);
});
chrome.runtime.onStartup.addListener(connect);
connect();
