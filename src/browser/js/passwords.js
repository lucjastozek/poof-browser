// Login detection for the password manager. Runs in the top frame of every page.
// It only reports two things to the browser: "this page has a password field" (so saved
// logins can be filled) and "the user submitted a login" (so it can offer to save it).
// It never fills anything itself and never touches the page's own forms.
(function () {
  if (window.top !== window) return;

  function post(message) {
    try {
      window.ipc.postMessage(JSON.stringify(message));
    } catch (e) {}
  }

  function visible(el) {
    var r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== "hidden";
  }

  function passwordFields(root) {
    return Array.prototype.filter.call(
      root.querySelectorAll("input[type=password]"),
      visible,
    );
  }

  // The username is the closest visible text-like field before the password field.
  function usernameBefore(password) {
    var scope = password.form || document;
    var candidates = scope.querySelectorAll(
      "input:not([type]),input[type=text],input[type=email],input[type=tel]",
    );
    var found = "";
    Array.prototype.forEach.call(candidates, function (input) {
      var before = input.compareDocumentPosition(password) & Node.DOCUMENT_POSITION_FOLLOWING;
      if (before && visible(input) && input.value) found = input.value;
    });
    return found;
  }

  var lastSent = "";
  function submitted(password) {
    var key = usernameBefore(password) + "\u0000" + password.value;
    if (!password.value || key === lastSent) return;
    lastSent = key;
    setTimeout(function () {
      lastSent = "";
    }, 1500);
    post({ t: "pw_submit", u: usernameBefore(password), p: password.value });
  }

  function filledPasswordIn(root) {
    return passwordFields(root).find(function (field) {
      return field.value;
    });
  }

  addEventListener(
    "submit",
    function (e) {
      if (!e.isTrusted || !e.target || !e.target.querySelectorAll) return;
      var field = filledPasswordIn(e.target);
      if (field) submitted(field);
    },
    true,
  );

  // Sites that log in with fetch() never fire "submit": Enter in the field, or a click on a
  // button next to it, is the signal.
  addEventListener(
    "keydown",
    function (e) {
      if (!e.isTrusted || e.key !== "Enter") return;
      var t = e.target;
      if (t && t.type === "password" && t.value) submitted(t);
    },
    true,
  );
  addEventListener(
    "click",
    function (e) {
      if (!e.isTrusted || !e.target || !e.target.closest) return;
      var button = e.target.closest("button,input[type=submit],[role=button]");
      var scope = button;
      for (var depth = 0; scope && depth < 5; depth++) {
        scope = scope.parentElement;
        var field = scope && filledPasswordIn(scope);
        if (field) return submitted(field);
      }
    },
    true,
  );

  // Tell the browser once per address that a login form is here. Event-driven: no observers or
  // timers, so pages without a login cost nothing.
  var announced = "";
  function announce() {
    if (announced === location.href || !passwordFields(document).length) return;
    announced = location.href;
    post({ t: "pw_form" });
  }
  addEventListener("DOMContentLoaded", announce);
  addEventListener("load", function () {
    announce();
    setTimeout(announce, 1500);
  });
  addEventListener("pageshow", announce);
  addEventListener("focusin", announce, true);
})();
