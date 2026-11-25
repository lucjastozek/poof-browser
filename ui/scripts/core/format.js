/* ---------- text formatting shared by the history and downloads views ---------- */
/* Host of an address without a leading "www."; the input itself when it is not an address. */
function hostOf(u) {
  try {
    return new URL(u).host.replace(/^www\./, "");
  } catch (_) {
    return u;
  }
}
/* "Today", "Yesterday" or a long date, for a Unix time in seconds. */
function dayLabel(ts) {
  const d = new Date(ts * 1000);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const day = new Date(d);
  day.setHours(0, 0, 0, 0);
  const diff = Math.round((today - day) / 86400000);
  if (diff === 0) return "Today";
  if (diff === 1) return "Yesterday";
  return d.toLocaleDateString(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: d.getFullYear() !== today.getFullYear() ? "numeric" : undefined,
  });
}
/* "1.4 MB": decimal units, one decimal below 10 and none above. */
function formatBytes(n) {
  if (!Number.isFinite(n) || n < 0) return "";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1000 && i < units.length - 1) {
    n /= 1000;
    i++;
  }
  return (n < 10 && i > 0 ? n.toFixed(1) : Math.round(n)) + " " + units[i];
}
/* "2 min left" from a number of seconds. */
function formatTimeLeft(seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) return "";
  if (seconds < 5) return "A few seconds left";
  if (seconds < 60) return Math.round(seconds) + " sec left";
  if (seconds < 3600) return Math.round(seconds / 60) + " min left";
  const h = Math.floor(seconds / 3600);
  const m = Math.round((seconds % 3600) / 60);
  return h + " h" + (m ? " " + m + " min" : "") + " left";
}
