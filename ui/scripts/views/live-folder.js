/* ---------- live folder dialog: create or edit a folder fed by a feed or a GitHub search ---------- */
const LIVE_INTERVALS = [
  [15, "15 minutes"],
  [30, "30 minutes"],
  [60, "1 hour"],
  [180, "3 hours"],
  [720, "12 hours"],
  [1440, "Once a day"],
];
const GITHUB_EXAMPLES = [
  ["Pull requests of a repository", "is:pr is:open repo:owner/name"],
  ["Review requests", "is:pr is:open review-requested:username"],
  ["Assigned issues", "is:issue is:open assignee:username"],
];

/* A typed address without a scheme means https. */
const withScheme = (s) => (/^[a-z][a-z0-9+.-]*:/i.test(s) ? s : "https://" + s);

function buildLiveFolder(v, target) {
  const add = target === "add";
  const folder = add
    ? null
    : (S.folders || []).find((f) => f.id === target && f.live);
  if (!add && !folder) {
    closeView();
    return;
  }
  const live = folder && folder.live;
  let kind = live ? live.source.kind : "feed";

  const name = el("input", "inp");
  name.maxLength = 40;
  name.placeholder = "Name (optional)";
  name.setAttribute("aria-label", "Folder name");
  name.value = folder ? folder.name : "";

  const address = el("input", "inp");
  address.placeholder = "https://example.com/feed.xml";
  address.setAttribute("aria-label", "Feed address");
  address.value = live && kind === "feed" ? live.source.url : "";

  const query = el("input", "inp");
  query.maxLength = 256;
  query.placeholder = "is:pr is:open repo:owner/name";
  query.setAttribute("aria-label", "GitHub search");
  query.value = live && kind === "github" ? live.source.query : "";

  const examples = el("div", "live-examples");
  GITHUB_EXAMPLES.forEach(([label, text]) =>
    examples.append(
      btn("sm ghost", label, () => {
        query.value = text;
        query.focus();
      }),
    ),
  );
  const feedBox = field(
    "Feed address",
    address,
    el(
      "div",
      "note",
      "An RSS or Atom feed. The newest entries appear as links in the folder.",
    ),
  );
  const githubBox = field(
    "GitHub search",
    query,
    examples,
    el(
      "div",
      "note",
      "Searches public GitHub. Replace words like owner/name and username with real ones.",
    ),
  );

  const interval = el("select", "engine-select");
  interval.setAttribute("aria-label", "Refresh interval");
  LIVE_INTERVALS.forEach(([minutes, text]) => {
    const o = el("option", null, text);
    o.value = String(minutes);
    o.selected = minutes === (live ? live.interval_minutes : 30);
    interval.append(o);
  });
  // An interval saved by an older version may not be in the list.
  if (live && !LIVE_INTERVALS.some(([m]) => m === live.interval_minutes)) {
    const o = el("option", null, live.interval_minutes + " minutes");
    o.value = String(live.interval_minutes);
    o.selected = true;
    interval.append(o);
  }

  const show = () => {
    feedBox.hidden = kind !== "feed";
    githubBox.hidden = kind !== "github";
  };
  const source = segmented(
    [
      ["feed", "RSS / Atom"],
      ["github", "GitHub"],
    ],
    kind,
    (value) => {
      kind = value;
      show();
    },
    "Source",
  );

  const save = () => {
    const text = kind === "feed" ? address.value.trim() : query.value.trim();
    if (!text) {
      window.__toast(
        kind === "feed" ? "Enter the feed address" : "Enter a GitHub search",
      );
      (kind === "feed" ? address : query).focus();
      return;
    }
    const body = {
      name: name.value.trim(),
      source:
        kind === "feed"
          ? { kind, url: withScheme(text) }
          : { kind, query: text },
      interval_minutes: Number(interval.value),
    };
    go(
      add
        ? { cmd: "create_live_folder", ...body }
        : { cmd: "edit_live_folder", id: folder.id, ...body },
    );
  };

  dialog(
    v,
    add ? "New live folder" : "Edit live folder",
    [
      field("Name", name),
      field("Source", source),
      feedBox,
      githubBox,
      field("Refresh every", interval),
    ],
    [
      el("span", "grow"),
      btn("ghost", "Cancel", closeView),
      btn("primary", add ? "Create folder" : "Save", save),
    ],
  );
  show();
  for (const input of [name, address, query])
    input.onkeydown = (e) => {
      if (e.key === "Enter") save();
    };
  (add ? address : name).focus();
}
