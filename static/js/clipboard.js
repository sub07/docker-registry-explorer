"use strict";

function copyToClipboard(element) {
  const image = element.dataset.image;
  const clipboard = navigator.clipboard;
  clipboard.writeText(image);
  element.textContent = "\u{2713}";
}
