/* 侧边栏宽度可拖拽调整：拖动右边缘左右拉伸，双击恢复默认，宽度记在 localStorage。 */
(function () {
  var MIN = 180, MAX = 640, DEFAULT = 17, KEY = 'foa-sidebar-width';
  var root = document.documentElement;
  var dragging = false;

  try {
    var saved = window.localStorage.getItem(KEY);
    var v = saved ? parseInt(saved, 10) : NaN;
    if (!isNaN(v) && v >= MIN && v <= MAX) {
      root.style.setProperty('--sidebar-width', v + 'px');
    }
  } catch (e) {}

  function sidebar() { return document.querySelector('.sidebar'); }

  var handle = document.createElement('div');
  handle.className = 'sidebar-resizer';
  handle.title = '拖动调整侧边栏宽度（双击恢复默认）';
  handle.setAttribute('aria-label', '调整侧边栏宽度');
  handle.setAttribute('role', 'separator');
  handle.style.display = 'none';

  function ready(fn) {
    if (document.body) { fn(); } else { document.addEventListener('DOMContentLoaded', fn); }
  }
  ready(function () { document.body.appendChild(handle); place(); });

  function place() {
    var el = sidebar();
    if (!el || dragging) { return; }
    var r = el.getBoundingClientRect();
    var hidden = r.right <= 4 || el.classList.contains('hidden') || window.innerWidth <= 768;
    if (hidden) { handle.style.display = 'none'; return; }
    handle.style.display = 'block';
    handle.style.left = Math.round(r.right - 5) + 'px';
  }

  function clamp(px) { return Math.min(MAX, Math.max(MIN, px)); }

  function apply(px) {
    root.style.setProperty('--sidebar-width', clamp(px) + 'px');
    var el = sidebar();
    if (el) {
      var r = el.getBoundingClientRect();
      handle.style.left = Math.round(r.right - 5) + 'px';
    }
  }

  function save(px) {
    try { window.localStorage.setItem(KEY, px + 'px'); } catch (e) {}
  }

  function onMove(e) {
    if (!dragging) { return; }
    var x = e.touches ? e.touches[0].clientX : e.clientX;
    apply(x);
    e.preventDefault();
  }

  function stopDrag() {
    if (!dragging) { return; }
    dragging = false;
    handle.classList.remove('active');
    document.body.classList.remove('sidebar-resizing');
    var el = sidebar();
    if (el) { save(Math.round(el.getBoundingClientRect().width)); }
    place();
  }

  function startDrag(e) {
    dragging = true;
    handle.classList.add('active');
    document.body.classList.add('sidebar-resizing');
    e.preventDefault();
  }

  handle.addEventListener('mousedown', startDrag);
  handle.addEventListener('touchstart', startDrag, { passive: false });
  handle.addEventListener('dblclick', function () {
    root.style.removeProperty('--sidebar-width');
    try { window.localStorage.removeItem(KEY); } catch (e) {}
    window.requestAnimationFrame(place);
  });

  window.addEventListener('mousemove', onMove);
  window.addEventListener('touchmove', onMove, { passive: false });
  window.addEventListener('mouseup', stopDrag);
  window.addEventListener('touchend', stopDrag);
  window.addEventListener('blur', stopDrag);

  window.addEventListener('resize', place);
  window.addEventListener('hashchange', function () { window.setTimeout(place, 60); });
  window.addEventListener('load', place);

  if (window.MutationObserver) {
    new window.MutationObserver(place).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['class', 'style'] });
  }
  window.setInterval(place, 1500);
})();
