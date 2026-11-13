// Vitrine Studio website – the only script. No dependencies, no tracking.
// Navigation toggle, theme switch, header shadow, scroll reveal, section
// index, tabs, a gentle pointer tilt of the hero canvas.
(function () {
  'use strict';
  var root = document.documentElement;
  var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // ---------- Mobile navigation ----------
  var toggle = document.querySelector('[data-nav-toggle]');
  var nav = document.getElementById('site-nav');
  if (toggle && nav) {
    toggle.addEventListener('click', function () {
      var open = nav.classList.toggle('is-open');
      toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
    });
    nav.addEventListener('click', function (e) {
      if (e.target.closest('a') && nav.classList.contains('is-open')) {
        nav.classList.remove('is-open');
        toggle.setAttribute('aria-expanded', 'false');
      }
    });
    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && nav.classList.contains('is-open')) {
        nav.classList.remove('is-open');
        toggle.setAttribute('aria-expanded', 'false');
        toggle.focus();
      }
    });
  }

  // ---------- "Home" in the header: on the home page, glide back to the top ----------
  var homeLink = document.querySelector('[data-home-link][aria-current="page"]');
  if (homeLink) {
    homeLink.addEventListener('click', function (e) {
      if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      e.preventDefault();
      window.scrollTo({ top: 0, behavior: reduceMotion ? 'auto' : 'smooth' });
      if (location.hash) history.replaceState(null, '', location.pathname + location.search);
    });
  }

  // ---------- Docs contents: collapsed on narrow screens ----------
  var docsNav = document.querySelector('[data-docs-nav]');
  if (docsNav && window.matchMedia('(max-width: 900px)').matches) docsNav.open = false;

  // ---------- Theme: explicit choice in localStorage (no cookie) ----------
  var themeBtn = document.querySelector('[data-theme-toggle]');
  if (themeBtn) {
    themeBtn.addEventListener('click', function () {
      var current = root.dataset.theme ||
        (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
      var next = current === 'dark' ? 'light' : 'dark';
      root.dataset.theme = next;
      try { localStorage.setItem('theme', next); } catch (e) { /* private mode */ }
    });
  }

  // ---------- Header shadow once the page scrolls ----------
  var header = document.querySelector('[data-header]');
  if (header) {
    var onScroll = function () { header.classList.toggle('is-scrolled', window.scrollY > 8); };
    window.addEventListener('scroll', onScroll, { passive: true });
    onScroll();
  }

  // ---------- Scroll reveal ----------
  var reveals = document.querySelectorAll('.reveal');
  if (!reduceMotion && 'IntersectionObserver' in window) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          io.unobserve(entry.target);
        }
      });
    }, { rootMargin: '0px 0px -10% 0px', threshold: 0.08 });
    reveals.forEach(function (el) { io.observe(el); });
  } else {
    reveals.forEach(function (el) { el.classList.add('is-visible'); });
  }

  // ---------- Section index (home page, right edge) ----------
  // The current section is the last one whose content (the top of the
  // section plus its padding, where the heading starts) has reached the
  // upper part of the viewport below the sticky header; at the very bottom
  // the last section that is on screen wins. The rail fills up to the
  // current tick and on towards the next as the reader moves between them.
  // Shown all the time on wide screens once the hero has left, everywhere
  // while scrolling (fades out shortly after scrolling stops).
  // Own scope: the hero tilt below has its own `update`.
  (function (secnav) {
    if (!secnav) return;
    var links = Array.prototype.slice.call(secnav.querySelectorAll('a[href^="#"]'));
    var targets = links.map(function (a) { return document.getElementById(a.hash.slice(1)); });
    var siteHeader = document.querySelector('[data-header]');
    var wideMQ = window.matchMedia('(min-width: 1100px)');
    var hero = targets[0];
    var current = -1, scrolling = false, open = false, hovering = false, scrollTimer = 0, heroGone = false;
    var lastPointer = 'mouse';
    secnav.hidden = false;

    var contentTop = function (el) {
      if (!el) return Infinity;
      return el.getBoundingClientRect().top + (parseFloat(getComputedStyle(el).paddingTop) || 0);
    };
    var setCurrent = function (index) {
      if (index === current) return;
      current = index;
      links.forEach(function (a, i) {
        if (i === index) a.setAttribute('aria-current', 'location');
        else a.removeAttribute('aria-current');
        a.classList.toggle('is-past', i < index);
        a.dataset.d = String(Math.min(Math.abs(i - index), 3));
      });
    };
    var setShown = function () {
      secnav.classList.toggle('is-shown', open || scrolling || hovering || (wideMQ.matches && heroGone));
      secnav.classList.toggle('is-scrolling', scrolling && !open);
      secnav.classList.toggle('is-open', open);
    };
    // Called directly from the scroll event (browsers fire it once per frame);
    // a few rects per call, no layout writes before the reads.
    var update = function () {
      var headerH = siteHeader ? siteHeader.getBoundingClientRect().height : 0;
      var line = headerH + (window.innerHeight - headerH) * 0.3;
      var tops = targets.map(contentTop);
      heroGone = !hero || hero.getBoundingClientRect().bottom < window.innerHeight * 0.5;
      var idx = 0;
      tops.forEach(function (t, i) { if (t <= line) idx = i; });
      var frac = idx < tops.length - 1 ? (line - tops[idx]) / (tops[idx + 1] - tops[idx]) : 0;
      frac = Math.min(Math.max(frac, 0), 1);
      var max = document.documentElement.scrollHeight - window.innerHeight;
      if (window.scrollY >= max - 2) {
        // Bottom of the page: the last headings may never reach the line.
        tops.forEach(function (t, i) { if (t < window.innerHeight - 40 && i > idx) { idx = i; frac = 0; } });
      }
      setCurrent(idx);
      secnav.style.setProperty('--progress', ((idx + frac) / Math.max(tops.length - 1, 1)).toFixed(4));
      setShown();
    };
    window.addEventListener('scroll', function () {
      scrolling = true;
      clearTimeout(scrollTimer);
      scrollTimer = setTimeout(function () { scrolling = false; setShown(); }, 1400);
      update();
    }, { passive: true });
    window.addEventListener('resize', update, { passive: true });
    // Images, fonts and the gallery cards move the sections without a scroll.
    window.addEventListener('load', update);
    if ('ResizeObserver' in window) new ResizeObserver(function () { update(); }).observe(document.body);
    update();

    // A mouse over the index keeps it from fading out under the pointer.
    secnav.addEventListener('pointerenter', function (e) { if (e.pointerType === 'mouse') { hovering = true; setShown(); } });
    secnav.addEventListener('pointerleave', function () { if (hovering) { hovering = false; setShown(); } });

    // Touch (tablets): a tap on a visible label jumps; a tap on the rail
    // opens the card with all labels; a tap elsewhere or Escape closes it.
    var setOpen = function (value) { open = value; setShown(); };
    secnav.addEventListener('pointerdown', function (e) { lastPointer = e.pointerType; });
    secnav.addEventListener('click', function (e) {
      var a = e.target.closest('a');
      if (!a) return;
      if (e.detail > 0 && lastPointer !== 'mouse' && !open && e.target.closest('.secnav__tick')) {
        e.preventDefault();
        setOpen(true);
        return;
      }
      if (open) setOpen(false);
      if (e.detail > 0) a.blur();
    });
    document.addEventListener('click', function (e) {
      if (open && !secnav.contains(e.target)) setOpen(false);
    });
    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && open) setOpen(false);
    });
  })(document.querySelector('[data-secnav]'));

  // ---------- Tabs (showcase) ----------
  document.querySelectorAll('[data-tabs]').forEach(function (box) {
    var tabs = Array.prototype.slice.call(box.querySelectorAll('[role="tab"]'));
    function select(tab, focus) {
      tabs.forEach(function (t) {
        var on = t === tab;
        t.setAttribute('aria-selected', on ? 'true' : 'false');
        t.tabIndex = on ? 0 : -1;
        var panel = document.getElementById(t.getAttribute('aria-controls'));
        if (panel) panel.hidden = !on;
      });
      if (focus) tab.focus();
    }
    tabs.forEach(function (tab, i) {
      tab.addEventListener('click', function () { select(tab, false); });
      tab.addEventListener('keydown', function (e) {
        var j = null;
        if (e.key === 'ArrowRight') j = (i + 1) % tabs.length;
        if (e.key === 'ArrowLeft') j = (i - 1 + tabs.length) % tabs.length;
        if (e.key === 'Home') j = 0;
        if (e.key === 'End') j = tabs.length - 1;
        if (j !== null) { e.preventDefault(); select(tabs[j], true); }
      });
    });
  });

  // ---------- Hero: flatten the canvas while scrolling, tilt with the pointer ----------
  var stage = document.querySelector('[data-tilt]');
  if (stage && !reduceMotion && window.matchMedia('(min-width: 641px)').matches) {
    var baseX = 8, px = 0, py = 0, ticking = false;
    var update = function () {
      ticking = false;
      var rect = stage.getBoundingClientRect();
      var progress = Math.min(Math.max(1 - rect.top / (window.innerHeight * 0.75), 0), 1);
      var x = baseX * (1 - progress) - py * 4;
      stage.style.setProperty('--tilt-x', x.toFixed(2) + 'deg');
      stage.style.setProperty('--tilt-y', (px * 5).toFixed(2) + 'deg');
    };
    var request = function () { if (!ticking) { ticking = true; requestAnimationFrame(update); } };
    window.addEventListener('scroll', request, { passive: true });
    stage.addEventListener('pointermove', function (e) {
      var r = stage.getBoundingClientRect();
      px = (e.clientX - r.left) / r.width - 0.5;
      py = (e.clientY - r.top) / r.height - 0.5;
      request();
    });
    stage.addEventListener('pointerleave', function () { px = 0; py = 0; request(); });
    update();
  }
})();
