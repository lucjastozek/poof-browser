// Ask the native side to restore the current space only after this UI is ready
// to receive its first state snapshot. The local time lets it apply the space schedule.
window.addEventListener(
  "load",
  () => {
    const d = new Date();
    send({ cmd: "ready", local_min: d.getHours() * 60 + d.getMinutes() });
  },
  { once: true },
);
