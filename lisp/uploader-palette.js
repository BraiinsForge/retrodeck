
(function () {
  "use strict";
  var pickers = document.querySelectorAll("[data-palette-picker]");
  for (var index = 0; index < pickers.length; index += 1) {
    (function (picker) {
      var text = document.getElementById(picker.getAttribute("data-palette-picker"));
      if (!text) return;
      picker.addEventListener("input", function () {
        text.value = picker.value.toUpperCase();
      });
      text.addEventListener("input", function () {
        if (/^#[0-9A-Fa-f]{6}$/.test(text.value)) {
          picker.value = text.value;
        }
      });
      text.addEventListener("blur", function () {
        text.value = text.value.toUpperCase();
      });
    }(pickers[index]));
  }
}());
