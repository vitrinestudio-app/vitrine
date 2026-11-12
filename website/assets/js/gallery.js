// Vitrine Studio website – template gallery (/gallery/) and the live cards
// of the home teaser. No dependencies, no cookies, no third parties: the
// only requests go to our own API (params.galleryAPI).
//
// Data: a build-time snapshot (#gallery-data) renders the page; the script
// then refreshes it live from GET /v1/templates (+ /v1/tags) and filters in
// the browser. Cards show one canvas of a template like a store gallery:
// pages sliced from one sprite, floating, tilting toward the pointer.
// Filters animate with FLIP; the URL query mirrors them, #<id> opens the
// detail view. docs/api/API.md describes the objects.
(function () {
  'use strict';

  var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var finePointer = window.matchMedia('(hover: hover) and (pointer: fine)').matches;
  var FACETS = ['style', 'color', 'use'];
  var MAX_ANIMATED = 40;

  // ---------- small helpers ----------
  function readJson(id) {
    var node = document.getElementById(id);
    if (!node) return null;
    try { return JSON.parse(node.textContent); } catch (e) { return null; }
  }
  function h(tag, cls, text) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text != null) n.textContent = text;
    return n;
  }
  function fmt(s, vars) {
    return String(s || '').replace(/\{(\w+)\}/g, function (m, k) { return k in vars ? vars[k] : m; });
  }
  function clamp(v, a, b) { return Math.min(Math.max(v, a), b); }
  function svg(paths) {
    return '<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" ' +
      'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' + paths + '</svg>';
  }
  var DEVICE_ICONS = {
    phone: '<rect x="7" y="2.5" width="10" height="19" rx="2.6"/><path d="M11 18.6h2"/>',
    tablet: '<rect x="4" y="3" width="16" height="18" rx="2.4"/><path d="M11 18h2"/>',
    watch: '<rect x="6.5" y="6" width="11" height="12" rx="3"/><path d="M9 6l.8-3.5h4.4L15 6M9 18l.8 3.5h4.4L15 18"/>',
    mac: '<rect x="4" y="5" width="16" height="11" rx="1.6"/><path d="M2 19h20"/>',
    web: '<rect x="3" y="4" width="18" height="16" rx="2.4"/><path d="M3 8.5h18M6.2 6.3h.01M8.7 6.3h.01"/>',
    tv: '<rect x="2.5" y="4.5" width="19" height="12.5" rx="2"/><path d="M8 20.5h8"/>',
    banner: '<rect x="2.5" y="7" width="19" height="10" rx="2"/><path d="M6 12h7"/>'
  };

  // ---------- canvases, sprites ----------
  // Per-canvas devices come from the API (`canvases[].devices`); servers
  // before 2026-11-12 lack them, so guess from the canvas and target ids.
  var DEVICE_HINTS = [
    ['watch', /watch/], ['tablet', /ipad|tablet/], ['tv', /(^|[-_ ])tv([-_ ]|$)|appletv/],
    ['mac', /(^|[-_ ])mac/], ['web', /(^|[-_ ])web|browser/],
    ['banner', /banner|header|feature|graphic|social|blog/], ['phone', /phone/]
  ];
  function canvasDevices(c) {
    if (c.devices && c.devices.length) return c.devices;
    var hay = [c.id].concat(c.targets || []).join(' ').toLowerCase();
    return DEVICE_HINTS.filter(function (d) { return d[1].test(hay); }).map(function (d) { return d[0]; });
  }
  function pickCanvas(t, device) {
    var list = t.canvases || [];
    var want = device || 'phone';
    for (var i = 0; i < list.length; i++) if (canvasDevices(list[i]).indexOf(want) >= 0) return list[i];
    return list[0] || null;
  }
  function ratioOf(c) {
    var s = c.sprite;
    if (s && s.page_width && s.page_height) return s.page_width / s.page_height;
    var m = String(c.page_aspect || c.aspect || '1:2').split(':');
    var r = parseFloat(m[0]) / parseFloat(m[1]);
    return r > 0 ? r : 0.5;
  }

  // Lazy sprites: a stage gets its image when it comes near the viewport;
  // the pages show a skeleton until the image is decoded.
  function loadStage(stage, large) {
    if (stage.classList.contains('is-loaded') || stage.dataset.loading) return;
    var url = stage.dataset.sprite;
    if (!url) return;
    // The 1× sprite has 600 px pages: enough for cards even on retina;
    // the detail view takes the 2× sprite when the pages are taller.
    if (large && stage.dataset.sprite2x) {
      var need = (stage.clientHeight || 400) * (window.devicePixelRatio || 1);
      if (need > (parseFloat(stage.dataset.pageHeight) || 600) * 1.15) url = stage.dataset.sprite2x;
    }
    stage.dataset.loading = '1';
    var img = new Image();
    img.decoding = 'async';
    var done = function () {
      stage.style.setProperty('--spr', 'url("' + url.replace(/"/g, '%22') + '")');
      stage.classList.add('is-loaded');
    };
    img.onload = function () { (img.decode ? img.decode() : Promise.resolve()).then(done, done); };
    img.onerror = function () { delete stage.dataset.loading; };
    img.src = url;
  }
  var lazy = 'IntersectionObserver' in window ? new IntersectionObserver(function (entries) {
    entries.forEach(function (e) {
      if (e.isIntersecting) { loadStage(e.target, false); lazy.unobserve(e.target); }
    });
  }, { rootMargin: '400px 0px' }) : null;
  function watchStage(stage) {
    if (lazy) lazy.observe(stage); else loadStage(stage, false);
    if (finePointer && !reduceMotion) bindTilt(stage, true);
    updateWide(stage);
  }
  function updateWide(stage) {
    var strip = stage.firstElementChild;
    if (strip) stage.classList.toggle('is-wide', strip.offsetWidth > stage.clientWidth + 2);
  }

  function buildStrip(c) {
    var n = Math.max(1, c.pages | 0);
    var strip = h('span', 'gcard__strip');
    for (var i = 0; i < n; i++) {
      var p = h('span', 'gp');
      p.style.setProperty('--i', i);
      p.style.setProperty('--x', (n > 1 ? (i * 100) / (n - 1) : 0) + '%');
      strip.appendChild(p);
    }
    return strip;
  }
  function styleStage(stage, c) {
    var n = Math.max(1, c.pages | 0);
    stage.style.setProperty('--n', n);
    stage.style.setProperty('--ar', String(ratioOf(c)));
    stage.dataset.canvas = c.id;
    if (c.sprite) {
      stage.dataset.sprite = c.sprite.url;
      if (c.sprite.url_2x) stage.dataset.sprite2x = c.sprite.url_2x;
      stage.dataset.pageHeight = c.sprite.page_height;
    }
  }
  function buildStage(t, c, href) {
    var a = h('a', 'gcard__stage');
    a.href = href;
    a.setAttribute('aria-label', fmt(S.galOpen, { name: t.name }));
    styleStage(a, c);
    a.appendChild(buildStrip(c));
    return a;
  }

  // ---------- pointer: tilt toward the pointer, pan through the pages ----------
  function bindTilt(stage, pan) {
    if (stage.dataset.tilt) return;
    stage.dataset.tilt = '1';
    var frame = 0, ev = null;
    var update = function () {
      frame = 0;
      var strip = stage.firstElementChild;
      if (!strip || !ev) return;
      var r = stage.getBoundingClientRect();
      var px = clamp((ev.clientX - r.left) / r.width, 0, 1);
      var py = clamp((ev.clientY - r.top) / r.height, 0, 1);
      var shift = 0;
      if (pan) {
        var over = strip.offsetWidth - r.width;
        shift = over > 0 ? -over * clamp((px - 0.12) / 0.76, 0, 1) : 0;
        strip.style.setProperty('--pan', shift.toFixed(1) + 'px');
      }
      strip.style.setProperty('--ry', ((px - 0.5) * 9).toFixed(2) + 'deg');
      strip.style.setProperty('--rx', ((0.5 - py) * 7).toFixed(2) + 'deg');
      var scroll = pan ? 0 : stage.scrollLeft;
      var pointerX = ev.clientX - r.left;
      var pages = strip.children;
      for (var i = 0; i < pages.length; i++) {
        var p = pages[i];
        // Offsets are relative to the stage (position: relative), untransformed.
        var mid = p.offsetLeft + p.offsetWidth / 2 + shift - scroll;
        var d = Math.abs(mid - pointerX) / Math.max(p.offsetWidth * 1.6, 60);
        p.style.setProperty('--lift', clamp(1 - d, 0, 1).toFixed(3));
      }
    };
    stage.addEventListener('pointermove', function (e) {
      if (e.pointerType !== 'mouse') return;
      ev = e;
      stage.classList.add('is-tracking');
      if (!frame) frame = requestAnimationFrame(update);
    });
    stage.addEventListener('pointerleave', function () {
      ev = null;
      stage.classList.remove('is-tracking');
      var strip = stage.firstElementChild;
      if (!strip) return;
      ['--pan', '--rx', '--ry'].forEach(function (k) { strip.style.removeProperty(k); });
      for (var i = 0; i < strip.children.length; i++) strip.children[i].style.removeProperty('--lift');
    });
  }

  // ---------- home teaser: motion + lazy sprites only ----------
  document.querySelectorAll('[data-gallery-teaser] .gcard__stage').forEach(watchStage);

  var root = document.querySelector('[data-gallery]');
  if (!root) return;

  // ================= gallery page =================
  var S = readJson('gallery-i18n') || {};
  var snapshot = readJson('gallery-data') || {};
  var api = (root.dataset.api || '').replace(/\/$/, '');
  var lang = root.dataset.lang || 'en';
  var grid = root.querySelector('[data-gal-grid]');
  var filtersBox = root.querySelector('[data-gal-filters]');
  var devicesBox = root.querySelector('[data-gal-devices]');
  var facetsBox = root.querySelector('[data-gal-facets]');
  var statusEl = root.querySelector('[data-gal-status]');
  var searchEl = root.querySelector('[data-gal-search]');
  var emptyEl = root.querySelector('[data-gal-empty]');
  var noMatchEl = root.querySelector('[data-gal-nomatch]');
  var clearBtns = root.querySelectorAll('[data-gal-clear]');

  var data = [];          // templates, API order (newest first)
  var byId = {};
  var cards = {};         // id → card element
  var shownCanvas = {};   // id → canvas id on the card
  var taxonomy = [];
  var labels = {};        // facet → { label, values: { id → label } }
  var state = { device: '', q: '', f: { style: {}, color: {}, use: {} } };
  var useOpen = false;
  var chips = {};         // facet → { value → button }
  var segs = {};          // device → button

  function label(obj) { return obj ? (obj[lang] || obj.en || '') : ''; }
  function setTaxonomy(facets) {
    taxonomy = facets || [];
    labels = {};
    taxonomy.forEach(function (f) {
      var vals = {};
      (f.values || []).forEach(function (v) { vals[v.id] = label(v.label) || v.id; });
      labels[f.id] = { label: label(f.label) || f.id, values: vals, order: (f.values || []).map(function (v) { return v.id; }) };
    });
  }
  function valueLabel(facet, v) { return (labels[facet] && labels[facet].values[v]) || v; }
  function tagsOf(t, facet) {
    if (facet === 'devices') return t.devices || [];
    return (t.tags && t.tags[facet]) || [];
  }

  // ---------- URL <-> state ----------
  function readUrl() {
    var q = new URLSearchParams(location.search);
    state.device = q.get('device') || '';
    state.q = q.get('q') || '';
    FACETS.forEach(function (f) {
      state.f[f] = {};
      (q.get(f) || '').split(',').forEach(function (v) { if (v) state.f[f][v] = true; });
    });
  }
  function writeUrl() {
    var q = new URLSearchParams();
    if (state.device) q.set('device', state.device);
    FACETS.forEach(function (f) {
      var vals = Object.keys(state.f[f]);
      if (vals.length) q.set(f, vals.join(','));
    });
    if (state.q) q.set('q', state.q);
    var s = q.toString().replace(/%2C/g, ',');
    history.replaceState(history.state, '', location.pathname + (s ? '?' + s : '') + location.hash);
  }
  function anyFilter() {
    return !!(state.device || state.q || FACETS.some(function (f) { return Object.keys(state.f[f]).length; }));
  }

  // ---------- matching ----------
  function matches(t, skip) {
    for (var i = 0; i < FACETS.length; i++) {
      var f = FACETS[i];
      if (f === skip) continue;
      var want = Object.keys(state.f[f]);
      if (want.length && !tagsOf(t, f).some(function (v) { return state.f[f][v]; })) return false;
    }
    if (skip !== 'devices' && state.device && (t.devices || []).indexOf(state.device) < 0) return false;
    if (state.q) {
      var hay = (t.name + ' ' + (t.description || '') + ' ' + (t.author || '') + ' ' +
        FACETS.map(function (f) { return tagsOf(t, f).map(function (v) { return v + ' ' + valueLabel(f, v); }).join(' '); }).join(' ')).toLowerCase();
      var words = state.q.toLowerCase().split(/\s+/).filter(Boolean);
      if (!words.every(function (w) { return hay.indexOf(w) >= 0; })) return false;
    }
    return true;
  }

  // ---------- filter UI ----------
  function valuesIn(facet) {
    var seen = {};
    data.forEach(function (t) { tagsOf(t, facet).forEach(function (v) { seen[v] = (seen[v] || 0) + 1; }); });
    var order = (labels[facet] && labels[facet].order) || [];
    var vals = Object.keys(seen);
    vals.sort(function (a, b) {
      if (facet === 'use' && seen[a] !== seen[b]) return seen[b] - seen[a];
      var ia = order.indexOf(a), ib = order.indexOf(b);
      return (ia < 0 ? 999 : ia) - (ib < 0 ? 999 : ib) || a.localeCompare(b);
    });
    return vals;
  }
  function chipContent(btn, text) {
    btn.textContent = '';
    btn.appendChild(document.createTextNode(text + ' '));
    btn.appendChild(h('span', 'gal-num'));
  }
  function buildFilters() {
    filtersBox.hidden = data.length === 0;
    // Devices: one choice ("what you view").
    devicesBox.textContent = '';
    segs = {};
    var all = h('button', 'gal-seg');
    all.type = 'button';
    all.innerHTML = '<span></span> <span class="gal-num"></span>';
    all.firstChild.textContent = S.galAllDevices || 'All';
    all.addEventListener('click', function () { setDevice(''); });
    devicesBox.appendChild(all);
    segs[''] = all;
    valuesIn('devices').forEach(function (d) {
      var b = h('button', 'gal-seg');
      b.type = 'button';
      b.innerHTML = svg(DEVICE_ICONS[d] || DEVICE_ICONS.phone) + '<span></span> <span class="gal-num"></span>';
      b.children[1].textContent = valueLabel('devices', d);
      b.addEventListener('click', function () { setDevice(state.device === d ? '' : d); });
      devicesBox.appendChild(b);
      segs[d] = b;
    });
    // Facets: multi-select chips with counts.
    facetsBox.textContent = '';
    chips = {};
    FACETS.forEach(function (f) {
      var vals = valuesIn(f);
      if (!vals.length) return;
      var row = h('div', 'gal-facet');
      row.appendChild(h('span', 'gal-facet__label', (labels[f] && labels[f].label) || f));
      var box = h('div', 'gal-chips');
      chips[f] = {};
      vals.forEach(function (v) {
        var b = h('button', 'gchip');
        b.type = 'button';
        chipContent(b, valueLabel(f, v));
        b.addEventListener('click', function () {
          if (state.f[f][v]) delete state.f[f][v]; else state.f[f][v] = true;
          changed(true);
        });
        box.appendChild(b);
        chips[f][v] = b;
      });
      if (f === 'use' && vals.length > 10) {
        var more = h('button', 'gchip gchip--more');
        more.type = 'button';
        more.dataset.more = '1';
        more.addEventListener('click', function () { useOpen = !useOpen; updateFilters(); });
        box.appendChild(more);
      }
      row.appendChild(box);
      facetsBox.appendChild(row);
    });
  }
  function updateFilters() {
    var count = function (skip, facet, v) {
      var n = 0;
      for (var i = 0; i < data.length; i++) if (tagsOf(data[i], facet).indexOf(v) >= 0 && matches(data[i], skip)) n++;
      return n;
    };
    Object.keys(segs).forEach(function (d) {
      var b = segs[d];
      var n = d ? count('devices', 'devices', d) : data.filter(function (t) { return matches(t, 'devices'); }).length;
      b.lastChild.textContent = n;
      b.setAttribute('aria-pressed', state.device === d ? 'true' : 'false');
      b.disabled = !n && state.device !== d;
    });
    FACETS.forEach(function (f) {
      if (!chips[f]) return;
      var vals = Object.keys(chips[f]);
      var hiddenCount = 0;
      vals.forEach(function (v, i) {
        var b = chips[f][v];
        var on = !!state.f[f][v];
        var n = count(f, f, v);
        b.lastChild.textContent = n;
        b.setAttribute('aria-pressed', on ? 'true' : 'false');
        b.disabled = !n && !on;
        var hide = f === 'use' && !useOpen && i >= 10 && !on;
        b.hidden = hide;
        if (hide) hiddenCount++;
      });
      var more = chips[f] && facetsBox.querySelector('.gchip--more');
      if (f === 'use' && more) {
        more.textContent = useOpen ? (S.galLess || 'Less') : fmt(S.galMore, { n: hiddenCount });
        more.hidden = !useOpen && hiddenCount === 0;
        more.setAttribute('aria-expanded', useOpen ? 'true' : 'false');
      }
    });
  }
  function setDevice(d) {
    state.device = d;
    changed(true);
  }

  // ---------- cards ----------
  function buildCard(t) {
    var c = pickCanvas(t, state.device);
    var card = h('article', 'gcard');
    card.id = t.id;
    card.dataset.id = t.id;
    if (c) card.appendChild(buildStage(t, c, '#' + t.id));
    var body = h('div', 'gcard__body');
    var title = h('h3', 'gcard__title');
    var link = h('a', null, t.name);
    link.href = '#' + t.id;
    title.appendChild(link);
    if (t.official) { title.appendChild(document.createTextNode(' ')); title.appendChild(h('span', 'gbadge', S.galOfficial)); }
    body.appendChild(title);
    if (t.description) body.appendChild(h('p', 'gcard__desc', t.description));
    var tags = h('ul', 'gcard__tags');
    FACETS.forEach(function (f) {
      tagsOf(t, f).forEach(function (v) { tags.appendChild(h('li', 'gtag gtag--' + f, valueLabel(f, v))); });
    });
    body.appendChild(tags);
    var meta = h('p', 'gcard__meta');
    meta.appendChild(h('span', 'gcard__pages', pagesText(c)));
    meta.appendChild(h('span', null, (t.devices || []).map(function (d) { return valueLabel('devices', d); }).join(' · ')));
    var dl = h('a', 'gcard__dl');
    dl.href = t.download_url;
    dl.rel = 'nofollow';
    dl.setAttribute('download', '');
    dl.innerHTML = svg('<path d="M12 3.5v12M7 11l5 5 5-5"/><path d="M4 17v1.5A2.5 2.5 0 0 0 6.5 21h11a2.5 2.5 0 0 0 2.5-2.5V17"/>') + '<span></span>';
    dl.lastChild.textContent = S.galDownloadShort;
    meta.appendChild(dl);
    body.appendChild(meta);
    card.appendChild(body);
    shownCanvas[t.id] = c ? c.id : '';
    return card;
  }
  function pagesText(c) {
    var n = c ? c.pages | 0 : 0;
    return n === 1 ? S.galPagesOne : fmt(S.galPages, { n: n });
  }
  function adoptCard(card) {
    var stage = card.querySelector('.gcard__stage');
    if (stage) watchStage(stage);
    card.addEventListener('click', onCardClick);
  }
  function onCardClick(e) {
    var a = e.target.closest('a');
    if (!a || a.classList.contains('gcard__dl')) return;
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.button > 0) return;
    e.preventDefault();
    openDetail(this.dataset.id, this.querySelector('.gcard__stage'), true);
  }

  // Swap the canvas a card shows (device filter), cross-fading.
  function swapCanvas(card, t, animate) {
    var c = pickCanvas(t, state.device);
    if (!c || shownCanvas[t.id] === c.id) return;
    shownCanvas[t.id] = c.id;
    var old = card.querySelector('.gcard__stage:not(.is-old)');
    var next = buildStage(t, c, '#' + t.id);
    var pages = card.querySelector('.gcard__pages');
    if (pages) pages.textContent = pagesText(c);
    if (old) old.after(next); else card.prepend(next);
    watchStage(next);
    if (animate) loadStage(next, false);
    if (!old) return;
    if (!animate || reduceMotion) { old.remove(); return; }
    old.classList.add('is-old');
    old.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 420, easing: 'ease' }).onfinish = function () { old.remove(); };
    next.animate([{ opacity: 0, transform: 'scale(.97)' }, { opacity: 1, transform: 'none' }], { duration: 520, easing: 'cubic-bezier(.2,.8,.2,1)' });
  }

  // ---------- FLIP: animate remaining, leaving and entering cards ----------
  function inView(r) {
    return r.bottom > -200 && r.top < window.innerHeight + 200 && r.width > 0;
  }
  function render(animate) {
    animate = animate && !reduceMotion && 'animate' in Element.prototype;
    var visible = data.filter(function (t) { return matches(t); });
    var keep = {};
    visible.forEach(function (t) { keep[t.id] = true; });

    var first = {};
    var gridRect = grid.getBoundingClientRect();
    if (animate) {
      Object.keys(cards).forEach(function (id) {
        var c = cards[id];
        if (!c.hidden && !c.classList.contains('is-leaving')) first[id] = c.getBoundingClientRect();
      });
    }
    // Leaving: take them out of the flow where they were, fade them out.
    var budget = MAX_ANIMATED;
    Object.keys(cards).forEach(function (id) {
      var c = cards[id];
      if (keep[id] || c.hidden) return;
      var r = first[id];
      if (animate && r && inView(r) && budget-- > 0) {
        c.classList.add('is-leaving');
        c.style.left = (r.left - gridRect.left) + 'px';
        c.style.top = (r.top - gridRect.top) + 'px';
        c.style.width = r.width + 'px';
        c.style.height = r.height + 'px';
        var anim = c.animate([{ opacity: 1, transform: 'none' }, { opacity: 0, transform: 'scale(.9) translateY(12px)' }],
          { duration: 380, easing: 'cubic-bezier(.4,0,.2,1)' });
        anim.onfinish = function () {
          if (!c.classList.contains('is-leaving')) return;
          c.hidden = true;
          c.classList.remove('is-leaving');
          c.style.left = c.style.top = c.style.width = c.style.height = '';
        };
      } else {
        c.hidden = true;
      }
    });
    // Entering and remaining: show, swap canvases, then invert + play.
    visible.forEach(function (t) {
      var c = cards[t.id];
      if (c.classList.contains('is-leaving')) {
        c.getAnimations().forEach(function (a) { a.cancel(); });
        c.classList.remove('is-leaving');
        c.style.left = c.style.top = c.style.width = c.style.height = '';
      }
      c.hidden = false;
    });
    visible.forEach(function (t) {
      var r = first[t.id];
      swapCanvas(cards[t.id], t, animate && !!r && inView(r));
    });
    if (animate) {
      budget = MAX_ANIMATED;
      visible.forEach(function (t, i) {
        var c = cards[t.id];
        var last = c.getBoundingClientRect();
        var r = first[t.id];
        if (!inView(last) && !(r && inView(r))) return;
        if (budget-- <= 0) return;
        if (r) {
          var dx = r.left - last.left, dy = r.top - last.top;
          if (Math.abs(dx) < 1 && Math.abs(dy) < 1) return;
          c.animate([{ transform: 'translate(' + dx + 'px,' + dy + 'px)' }, { transform: 'none' }],
            { duration: 620, easing: 'cubic-bezier(.2,.8,.2,1)' });
        } else {
          c.animate([{ opacity: 0, transform: 'scale(.92) translateY(18px)' }, { opacity: 1, transform: 'none' }],
            { duration: 560, delay: 120 + Math.min(i, 8) * 40, easing: 'cubic-bezier(.2,.8,.2,1)', fill: 'backwards' });
        }
      });
    }
    // Status.
    var total = data.length;
    statusEl.textContent = anyFilter()
      ? fmt(S.galShowing, { n: visible.length, total: total })
      : (total === 1 ? S.galCountOne : fmt(S.galCount, { n: total }));
    clearBtns[0].hidden = !anyFilter();
    noMatchEl.hidden = !(total > 0 && visible.length === 0);
    emptyEl.hidden = total > 0;
  }

  function changed(animate) {
    writeUrl();
    updateFilters();
    render(animate);
  }

  // ---------- data ----------
  function setData(list, animate) {
    data = list.slice();
    byId = {};
    data.forEach(function (t) { byId[t.id] = t; });
    // Reconcile cards (keyed by id), keep the API order.
    Object.keys(cards).forEach(function (id) {
      if (!byId[id]) { cards[id].remove(); delete cards[id]; delete shownCanvas[id]; }
    });
    data.forEach(function (t, i) {
      var c = cards[t.id];
      if (!c) {
        c = buildCard(t);
        c.style.setProperty('--k', i);
        c.hidden = true;   // render() shows it (entering)
        cards[t.id] = c;
        adoptCard(c);
      }
      grid.appendChild(c);
    });
    buildFilters();
    updateFilters();
    render(animate);
  }

  function signature(list) {
    return list.map(function (t) {
      return t.id + '@' + t.version + '@' + (t.canvases || []).map(function (c) { return c.sprite ? c.sprite.url : ''; }).join('|') + '@' + t.downloads;
    }).join(',');
  }

  function fetchJson(url) {
    return fetch(url, { mode: 'cors', credentials: 'omit' }).then(function (r) {
      if (!r.ok) throw new Error('HTTP ' + r.status);
      return r.json();
    });
  }
  function fetchAll() {
    var out = [];
    var page = function (offset, round) {
      return fetchJson(api + '/v1/templates?limit=200&offset=' + offset).then(function (j) {
        var list = j.templates || [];
        out = out.concat(list);
        var step = j.limit || 50;
        if (list.length < step || round > 20) return out;
        return page(offset + step, round + 1);
      });
    };
    return page(0, 0);
  }
  function refresh() {
    if (!api || !window.fetch) return;
    var tagsReady = taxonomy.length ? Promise.resolve() : fetchJson(api + '/v1/tags').then(function (j) { setTaxonomy(j.facets); }, function () {});
    Promise.all([fetchAll(), tagsReady]).then(function (res) {
      var live = res[0];
      if (signature(live) === signature(data)) return;
      setData(live, true);
      if (pendingHash) routeHash();
    }, function () { /* offline or no CORS: the snapshot stays */ });
  }

  // ---------- detail view ----------
  var dialog = document.querySelector('[data-gal-detail]');
  var D = {};
  if (dialog) dialog.querySelectorAll('[data-d]').forEach(function (n) { D[n.dataset.d] = n; });
  var detailId = '';
  var pushed = false;
  var pendingHash = false;

  function canvasLabel(t, c) {
    var devs = canvasDevices(c);
    var name = devs.length ? valueLabel('devices', devs[0]) : c.id;
    var landscape = ratioOf(c) > 1 && (devs[0] === 'phone' || devs[0] === 'tablet');
    var same = (t.canvases || []).filter(function (o) { return canvasDevices(o)[0] === devs[0]; }).length > 1;
    if (landscape) name += ' · ' + S.galLandscape;
    else if (same && !devs.length) name = c.id;
    return name + ' · ' + pagesText(c);
  }
  function showDetailCanvas(t, c) {
    var stage = D.stage;
    stage.textContent = '';
    stage.className = 'gdetail__stage';
    delete stage.dataset.loading;
    delete stage.dataset.sprite;
    delete stage.dataset.sprite2x;
    styleStage(stage, c);
    stage.appendChild(buildStrip(c));
    stage.scrollLeft = 0;
    D.canvases.querySelectorAll('button').forEach(function (b) {
      b.setAttribute('aria-pressed', b.dataset.canvas === c.id ? 'true' : 'false');
    });
    requestAnimationFrame(function () { loadStage(stage, true); updateArrows(); });
  }
  function updateArrows() {
    var s = D.stage;
    D.prev.disabled = s.scrollLeft < 8;
    D.next.disabled = s.scrollLeft + s.clientWidth > s.scrollWidth - 8;
  }
  function step(dir) {
    var p = D.stage.querySelector('.gp');
    if (!p) return;
    D.stage.scrollBy({ left: dir * (p.offsetWidth + 12), behavior: reduceMotion ? 'auto' : 'smooth' });
  }
  function fact(dl, k, v) {
    if (v === '' || v == null) return;
    var pair = h('div');
    pair.appendChild(h('dt', null, k));
    pair.appendChild(h('dd', null, v));
    dl.appendChild(pair);
  }
  function fillDetail(t) {
    D.name.textContent = t.name;
    D.desc.textContent = t.description || '';
    D.by.textContent = '';
    if (t.official) D.by.appendChild(h('span', 'gbadge', S.galOfficial));
    var who = t.author || t.uploader;
    if (who) D.by.appendChild(h('span', null, fmt(S.galBy, { name: who })));
    // Canvases.
    D.canvases.textContent = '';
    var list = t.canvases || [];
    if (list.length > 1) {
      list.forEach(function (c) {
        var b = h('button', 'gal-seg', canvasLabel(t, c));
        b.type = 'button';
        b.dataset.canvas = c.id;
        var dev = canvasDevices(c)[0];
        if (dev && DEVICE_ICONS[dev]) b.insertAdjacentHTML('afterbegin', svg(DEVICE_ICONS[dev]));
        b.addEventListener('click', function () { showDetailCanvas(t, c); });
        D.canvases.appendChild(b);
      });
    }
    // Tags.
    D.tags.textContent = '';
    FACETS.concat(['devices']).forEach(function (f) {
      var vals = tagsOf(t, f);
      if (!vals.length) return;
      var row = h('div');
      row.appendChild(h('h3', null, f === 'devices' ? S.galDevices : ((labels[f] && labels[f].label) || f)));
      var ul = h('ul');
      vals.forEach(function (v) { ul.appendChild(h('li', 'gtag gtag--' + f, valueLabel(f, v))); });
      row.appendChild(ul);
      D.tags.appendChild(row);
    });
    D.samples.textContent = t.samples > 0 ? fmt(S.galSamples, { n: t.samples }) : S.galNoSamples;
    var fonts = (t.fonts || []).filter(function (f, i, all) { return all.indexOf(f) === i; });
    D.fonts.textContent = fonts.length ? fmt(S.galFonts, { fonts: fonts.join(', ') }) : S.galNoFonts;
    D.facts.textContent = '';
    fact(D.facts, S.galVersion, t.version);
    fact(D.facts, S.galDownloads, t.downloads != null ? Number(t.downloads).toLocaleString(lang) : '');
    if (t.updated_at) fact(D.facts, S.galUpdated, new Date(t.updated_at * 1000).toLocaleDateString(lang, { year: 'numeric', month: 'short', day: 'numeric' }));
    fact(D.facts, S.galLicence, t.license || 'CC0-1.0');
    D.download.href = t.download_url;
    D.cli.textContent = 'vitrine gallery download ' + t.id + '\nvitrine new my-app --template ./' + t.id + '.vitrine-template';
    D.copy.lastChild.textContent = S.galCopyLink;
  }
  function openDetail(id, origin, push) {
    var t = byId[id];
    if (!dialog || !t) return;
    detailId = id;
    fillDetail(t);
    var c = pickCanvas(t, state.device);
    if (origin && !reduceMotion) {
      var r = origin.getBoundingClientRect();
      dialog.style.transformOrigin = (r.left + r.width / 2) + 'px ' + (r.top + r.height / 2) + 'px';
    }
    if (!dialog.open) dialog.showModal();
    dialog.scrollTop = 0;
    if (c) showDetailCanvas(t, c);
    if (push && location.hash !== '#' + id) {
      history.pushState({ gallery: id }, '', '#' + id);
      pushed = true;
    }
  }
  function closeDetail() {
    if (dialog && dialog.open) dialog.close();
  }
  if (dialog) {
    dialog.addEventListener('close', function () {
      detailId = '';
      if (location.hash) {
        if (pushed) { pushed = false; history.back(); }
        else history.replaceState(history.state, '', location.pathname + location.search);
      }
    });
    dialog.addEventListener('click', function (e) { if (e.target === dialog) closeDetail(); });
    dialog.querySelector('[data-gal-close]').addEventListener('click', closeDetail);
    D.prev.addEventListener('click', function () { step(-1); });
    D.next.addEventListener('click', function () { step(1); });
    D.stage.addEventListener('scroll', updateArrows, { passive: true });
    if (finePointer && !reduceMotion) bindTilt(D.stage, false);
    D.copy.addEventListener('click', function () {
      var url = location.origin + location.pathname + '#' + detailId;
      var ok = function () {
        D.copy.lastChild.textContent = S.galLinkCopied;
        setTimeout(function () { D.copy.lastChild.textContent = S.galCopyLink; }, 2200);
      };
      if (navigator.clipboard) navigator.clipboard.writeText(url).then(ok, function () {});
    });
  }
  function routeHash() {
    var id = decodeURIComponent(location.hash.slice(1));
    pendingHash = false;
    if (id && byId[id]) {
      if (detailId !== id) openDetail(id, cards[id] && cards[id].querySelector('.gcard__stage'), false);
    } else if (id && !byId[id]) {
      pendingHash = true;    // maybe in the live data
    } else if (dialog && dialog.open) {
      pushed = false;
      dialog.close();
    }
  }
  window.addEventListener('popstate', function () { pushed = false; routeHash(); });

  // ---------- wire up ----------
  setTaxonomy(snapshot.tags);
  readUrl();
  grid.querySelectorAll('.gcard').forEach(function (card) {
    cards[card.dataset.id] = card;
    var stage = card.querySelector('.gcard__stage');
    shownCanvas[card.dataset.id] = stage ? stage.dataset.canvas : '';
    card.classList.add('is-new');
    adoptCard(card);
  });
  setData(snapshot.templates || [], false);
  searchEl.value = state.q;
  var typing = 0;
  searchEl.addEventListener('input', function () {
    clearTimeout(typing);
    typing = setTimeout(function () { state.q = searchEl.value.trim(); changed(true); }, 160);
  });
  clearBtns.forEach(function (b) {
    b.addEventListener('click', function () {
      state.device = '';
      state.q = '';
      searchEl.value = '';
      FACETS.forEach(function (f) { state.f[f] = {}; });
      changed(true);
    });
  });
  window.addEventListener('resize', function () {
    grid.querySelectorAll('.gcard__stage').forEach(updateWide);
  }, { passive: true });
  routeHash();
  refresh();
})();
