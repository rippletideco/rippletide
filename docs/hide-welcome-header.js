(function () {
  function hideFirstSidebarGroupHeader() {
    var sidebar = document.getElementById('sidebar') || document.querySelector('[class*="sidebar"]');
    if (!sidebar) return;
    var groups = sidebar.querySelectorAll('[class*="sidebar-group"], [class*="SidebarGroup"]');
    if (!groups.length) return;
    var first = groups[0];
    var header = first.querySelector('[class*="sidebar-group-header"], [class*="SidebarGroupHeader"], [class*="group-header"]');
    function collapse(el) {
      el.style.setProperty('display', 'none', 'important');
      el.style.setProperty('height', '0', 'important');
      el.style.setProperty('margin', '0', 'important');
      el.style.setProperty('padding', '0', 'important');
      el.style.setProperty('overflow', 'hidden', 'important');
    }
    if (header) {
      collapse(header);
      return;
    }
    var children = first.children;
    for (var i = 0; i < children.length; i++) {
      var el = children[i];
      if (el.tagName === 'A' || el.querySelector('a')) continue;
      if (el.textContent.trim() === 'Welcome') {
        collapse(el);
        return;
      }
    }
    if (children.length && children[0].tagName !== 'A') {
      collapse(children[0]);
    }
    first.style.setProperty('padding-top', '0', 'important');
    first.style.setProperty('margin-top', '0', 'important');
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', hideFirstSidebarGroupHeader);
  } else {
    hideFirstSidebarGroupHeader();
  }
  setTimeout(hideFirstSidebarGroupHeader, 500);
  setTimeout(hideFirstSidebarGroupHeader, 1500);
})();
