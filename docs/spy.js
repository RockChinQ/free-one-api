/* 侧边栏高亮：严格跟随右侧滚动位置（修正 docsify 内置 scrollActive 在长页面底部滞后的问题） */
(function () {
  var OFFSET = 120, ticking = false, settle = null;

  function prefix() {
    var h = window.location.hash || '#/';
    var q = h.indexOf('?');
    return q < 0 ? h : h.slice(0, q);
  }

  function linkList() {
    var p = prefix();
    return [].slice.call(document.querySelectorAll('.sidebar a')).filter(function (a) {
      var h = a.getAttribute('href') || '';
      return h === p || h.indexOf(p + '?id=') === 0;
    });
  }

  function setActive(li, on) {
    if (!li) return;
    if (on) { li.classList.add('active'); } else { li.classList.remove('active'); }
  }

  function sync() {
    ticking = false;
    var links = linkList();
    if (!links.length) return;

    var items = [];
    links.forEach(function (a) {
      var h = a.getAttribute('href') || '';
      var i = h.indexOf('?id=');
      if (i < 0) { items.push([a, document.querySelector('.markdown-section')]); return; }
      var el = document.getElementById(decodeURIComponent(h.slice(i + 4)));
      if (el) items.push([a, el]);
    });
    if (!items.length) return;

    var atBottom = window.innerHeight + window.pageYOffset >= document.documentElement.scrollHeight - 4;
    var pick = items[0];
    if (atBottom) {
      pick = items[items.length - 1];
    } else {
      items.forEach(function (it) {
        if (it[1].getBoundingClientRect().top <= OFFSET) pick = it;
      });
    }

    links.forEach(function (a) {
      a.classList.remove('active');
      setActive(a.closest('li'), false);
    });
    pick[0].classList.add('active');
    var li = pick[0].closest('li');
    setActive(li, true);

    var box = document.querySelector('.sidebar');
    if (box && li) {
      var r = li.getBoundingClientRect();
      var b = box.getBoundingClientRect();
      if (r.top < b.top + 70 || r.bottom > b.bottom - 16) {
        box.scrollTop += (r.top - b.top) - b.height / 2 + r.height / 2;
      }
    }
  }

  function onScroll() {
    if (!ticking) { ticking = true; window.requestAnimationFrame(sync); }
    if (settle) window.clearTimeout(settle);
    settle = window.setTimeout(sync, 120);
  }

  function boot() {
    sync();
    window.setTimeout(sync, 300);
    window.setTimeout(sync, 1000);
  }

  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', onScroll, { passive: true });
  window.addEventListener('hashchange', boot);
  window.addEventListener('load', boot);
  document.addEventListener('DOMContentLoaded', boot);

  if (window.MutationObserver) {
    new window.MutationObserver(function () {
      if (settle) window.clearTimeout(settle);
      settle = window.setTimeout(sync, 150);
    }).observe(document.body, { childList: true, subtree: true });
  }

  window.setInterval(sync, 1500);
  boot();
})();
