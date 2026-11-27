// Page shortcuts that must keep working while the sidebar owns keyboard focus.
const isTyping = (el) =>
  !!el &&
  (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName));

document.addEventListener(
  "keydown",
  (e) => {
    if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;
    if (e.key === "Escape") {
      send({ cmd: "fullscreen_leave" });
    } else if (
      e.key.toLowerCase() === "f" &&
      !e.repeat &&
      !isTyping(e.target)
    ) {
      e.preventDefault();
      send({ cmd: "fullscreen_toggle" });
    }
  },
  true,
);
