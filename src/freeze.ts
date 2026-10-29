// Tauri's "freezePrototype" would do this in every webview, including the Jagex sign-in window, whose pages
// assign to built-in prototypes and render blank when they're frozen. So it's off in tauri.conf.json and the
// app's own window freezes it here instead, before any other module runs.
Object.freeze(Object.prototype);
