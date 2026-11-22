function build(name) {
  const v = $("stage");
  if (name === "newtab") buildNewTab(v);
  else if (name === "ext") buildExt(v);
  else if (name === "settings") buildSettings(v);
  else if (name.startsWith("space")) buildSpace(v, name.split(":")[1]);
}
