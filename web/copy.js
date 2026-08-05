/*
 * Putting a result on the clipboard, in the shape it has to be in when it lands.
 *
 * The bytes are the engine's. Nothing here writes Markdown: a block assembled in the tab would
 * be a second home for what a result is, and the two homes would drift the first time a rule
 * gained a value. This module is the button, the clipboard, and the answer to whether it worked.
 */

import { element } from './format.js';

/*
 * Writes text to the clipboard, and says whether it got there.
 *
 * `navigator.clipboard` needs a secure context, which a page served over plain HTTP from
 * anything but localhost is not. The fallback is the older selection route, because a reader
 * on such a page pressing Copy and receiving silence has been told nothing.
 */
async function put(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const holder = document.createElement('textarea');
    holder.value = text;
    holder.setAttribute('readonly', '');
    holder.style.cssText = 'position:fixed;top:-1000px';
    document.body.append(holder);
    holder.select();
    let copied = false;
    try {
      copied = document.execCommand('copy');
    } catch {
      copied = false;
    }
    holder.remove();
    return copied;
  }
}

/*
 * A button that copies what `produce` returns.
 *
 * `produce` is called at the moment of the press rather than when the button is built, so what
 * lands on the clipboard is the result on screen and not the one that was there when the panel
 * was drawn.
 *
 * The label says what happened for a moment and then goes back to what the button does. A
 * button that reported nothing leaves a reader pressing it twice; one that kept the report
 * leaves them unable to see what it does.
 */
export function copyButton(label, produce) {
  const button = element('button', 'button button--ghost button--small', label);
  button.type = 'button';
  let restore = null;
  button.addEventListener('click', async () => {
    let text = null;
    try {
      text = produce();
    } catch (raised) {
      text = null;
      button.textContent = String(raised?.message ?? raised).slice(0, 80);
    }
    if (text != null) {
      const copied = await put(text);
      button.textContent = copied ? 'Copied' : 'Could not reach the clipboard';
    }
    clearTimeout(restore);
    restore = setTimeout(() => { button.textContent = label; }, 2000);
  });
  return button;
}
