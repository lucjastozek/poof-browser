// A function expression; the browser calls it as (expected origin, username, password) with
// values it checked itself. It fills only the page's own login form, and only on that origin.
function (expectedOrigin, username, password) {
  if (location.origin !== expectedOrigin) return false;

  function visible(el) {
    var r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== "hidden";
  }
  function set(input, value) {
    var setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
    setter.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }

  var field = Array.prototype.find.call(
    document.querySelectorAll("input[type=password]"),
    function (input) {
      return visible(input) && !input.value;
    },
  );
  if (!field) return false;

  var scope = field.form || document;
  var user = null;
  Array.prototype.forEach.call(
    scope.querySelectorAll("input:not([type]),input[type=text],input[type=email],input[type=tel]"),
    function (input) {
      var before = input.compareDocumentPosition(field) & Node.DOCUMENT_POSITION_FOLLOWING;
      if (before && visible(input)) user = input;
    },
  );
  if (user && !user.value) set(user, username);
  set(field, password);
  return true;
}
