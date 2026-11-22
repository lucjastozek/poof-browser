// Ask the native side to restore the current space only after this UI is ready
// to receive its first state snapshot.
window.addEventListener("load", () => send({ cmd: "ready" }), { once: true });
