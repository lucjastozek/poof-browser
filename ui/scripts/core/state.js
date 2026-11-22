/* ---------- state ---------- */
let S = null,
  prevIdx = -1,
  ov = null,
  EXTS = [],
  lastKey = "",
  pending = null,
  drag = null,
  noClick = false;
const curSpace = () => S.spaces.find((x) => x.id === S.current);
