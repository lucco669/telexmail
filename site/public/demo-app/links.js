// Display link destinations without resolving them or contacting the sender's website.
(() => {
'use strict';
const hidden = /[\u0000-\u001f\u007f-\u009f\u00ad\u061c\u200b-\u200f\u202a-\u202e\u2060-\u206f\ufeff]/g;
const visible = value => String(value).replace(hidden, c => '\\u' + c.charCodeAt(0).toString(16).padStart(4, '0'));
function inspect(value){
  const href = String(value || '').trim();
  let url;
  try { url = new URL(href); } catch { return null; }
  if (!['http:', 'https:', 'mailto:'].includes(url.protocol)) return null;
  const web = url.protocol !== 'mailto:';
  if (web && !url.hostname) return null;
  return {href, display:visible(href), destination:web ? url.host : visible(url.pathname), web,
    credentials:!!(url.username || url.password), hidden:visible(href) !== href};
}
window.TelexLinks = Object.freeze({inspect, visible});
})();
