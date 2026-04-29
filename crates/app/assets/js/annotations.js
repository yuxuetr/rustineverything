/* RIE Annotation client runtime.
 *
 * 提供:
 *   window.RIE_ANNO.apply({ kind, path, items })  // 把已有标注覆盖到 DOM 上
 *   window.RIE_ANNO.captureSelection()             // 取选区 -> {block_id,start,end,exact,prefix,suffix}
 *   window.RIE_ANNO.submit(payload)                // POST /api/annotations/create
 *
 * 锚点策略：以最近的 [data-block-id] 祖先为基准；start/end 是该祖先 textContent 的字符偏移。
 * Markdown 渲染层尚未给顶层块注入 data-block-id 时，apply 会跳过对应条目（兼容退化）。
 */
(function () {
  if (window.RIE_ANNO) return; // 幂等

  // ---------- styles ----------
  const STYLE_CSS = `
.rie-anno { border-radius: 2px; padding: 0 1px; }
.rie-anno-yellow { background: rgba(234, 179, 8, 0.35); }
.rie-anno-green  { background: rgba(34, 197, 94, 0.30); }
.rie-anno-blue   { background: rgba(59, 130, 246, 0.30); }
.rie-anno-pink   { background: rgba(236, 72, 153, 0.30); }
.rie-anno-purple { background: rgba(168, 85, 247, 0.30); }
.rie-anno-underline { text-decoration: underline; text-decoration-thickness: 2px; text-underline-offset: 3px; }
.rie-anno-wavy { text-decoration: underline wavy; text-decoration-thickness: 2px; text-underline-offset: 3px; }
.rie-anno-strikethrough { text-decoration: line-through; text-decoration-thickness: 2px; }
/* 隐藏标注视图层（仅 CSS，不影响数据） */
body.no-anno .rie-anno {
  background: transparent !important;
  text-decoration: none !important;
}
.rie-anno-toolbar {
  position: absolute; z-index: 9999; display: flex; gap: 4px;
  padding: 4px 6px; border-radius: 8px; background: #0f172a; color: #fff;
  box-shadow: 0 8px 24px rgba(0,0,0,.25); font-size: 12px;
}
.rie-anno-toolbar button {
  width: 22px; height: 22px; border-radius: 4px; border: 0; cursor: pointer;
  display: inline-flex; align-items: center; justify-content: center; color: #fff;
}
.rie-anno-toolbar .rie-swatch-yellow { background: #eab308; }
.rie-anno-toolbar .rie-swatch-green  { background: #22c55e; }
.rie-anno-toolbar .rie-swatch-blue   { background: #3b82f6; }
.rie-anno-toolbar .rie-swatch-pink   { background: #ec4899; }
.rie-anno-toolbar .rie-swatch-purple { background: #a855f7; }
.rie-anno-toolbar .rie-style-underline,
.rie-anno-toolbar .rie-style-wavy,
.rie-anno-toolbar .rie-style-strike { background: #475569; font-weight: 700; }
.rie-anno-toolbar .rie-vis-select {
  height: 22px; border-radius: 4px; border: 0; padding: 0 4px;
  background: #1e293b; color: #fff; font-size: 11px; cursor: pointer;
}
.rie-anno-toolbar .rie-vis-select:focus { outline: 1px solid #60a5fa; }
/* 他人公开标注的轻量边框提示 */
.rie-anno.rie-anno-by-other {
  outline: 1px dashed rgba(100, 116, 139, 0.5);
  outline-offset: 1px;
}
/* 从 /me/annotations 跳回原文时的闪烁高亮 */
.rie-anno-flash {
  animation: rie-anno-flash-kf 1.6s ease-out 0s 2;
  border-radius: 4px;
}
@keyframes rie-anno-flash-kf {
  0%   { background-color: rgba(59, 130, 246, 0.35); box-shadow: 0 0 0 4px rgba(59, 130, 246, 0.25); }
  100% { background-color: transparent; box-shadow: 0 0 0 0 rgba(59, 130, 246, 0); }
}
`;
  function ensureStyles() {
    if (document.getElementById('rie-anno-styles')) return;
    const s = document.createElement('style');
    s.id = 'rie-anno-styles';
    s.textContent = STYLE_CSS;
    document.head.appendChild(s);
  }

  // ---------- helpers ----------
  function findBlock(blockId) {
    return document.querySelector(`[data-block-id="${cssEscape(blockId)}"]`);
  }
  function cssEscape(s) { return String(s).replace(/[^a-zA-Z0-9_-]/g, '\\$&'); }

  /** 以字符为单位在 root 内 walk text nodes；offset 落在某个 text 中时返回 {node, idx} */
  function locateOffset(root, offset) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
    let acc = 0;
    while (walker.nextNode()) {
      const node = walker.currentNode;
      const len = node.nodeValue.length;
      if (offset <= acc + len) return { node, idx: offset - acc };
      acc += len;
    }
    return null;
  }

  /** 用 Range 包裹 [start,end) 文本，加 class */
  function wrapRange(root, start, end, klass, dataAnnoId) {
    if (end <= start) return false;
    const a = locateOffset(root, start);
    const b = locateOffset(root, end);
    if (!a || !b) return false;
    const range = document.createRange();
    range.setStart(a.node, a.idx);
    range.setEnd(b.node, b.idx);
    const span = document.createElement('span');
    span.className = `rie-anno ${klass}`;
    if (dataAnnoId != null) span.setAttribute('data-anno-id', dataAnnoId);
    try { range.surroundContents(span); return true; }
    catch (_) { return false; } // 跨多个父节点时 surroundContents 会失败 — v1 忽略
  }

  function styleClass(style) {
    const known = ['yellow', 'green', 'blue', 'pink', 'purple',
                   'underline', 'wavy', 'strikethrough'];
    return known.includes(style) ? `rie-anno-${style}` : 'rie-anno-yellow';
  }

  // ---------- apply existing annotations ----------
  function apply(data) {
    if (!data || !Array.isArray(data.items)) return;
    ensureStyles();
    // 清除上一次注入的 anno（仅清除当前页 main 区域）
    document.querySelectorAll('span.rie-anno').forEach(el => {
      const parent = el.parentNode;
      while (el.firstChild) parent.insertBefore(el.firstChild, el);
      parent.removeChild(el);
      parent.normalize();
    });
    data.items.forEach(item => {
      const block = findBlock(item.block_id);
      if (!block) return;
      let cls = styleClass(item.style);
      // 他人公开标注额外加个轻量样式标记
      if (item.author_nickname) cls += ' rie-anno-by-other';
      const ok = wrapRange(block, item.start_offset, item.end_offset, cls, item.id);
      if (ok && item.author_nickname) {
        // 在刚创建的 span 上补 title（鼠标悬停显示作者）
        const span = block.querySelector(`span.rie-anno[data-anno-id="${cssEscape(item.id)}"]`);
        if (span) {
          const note = item.note ? ` · 备注: ${item.note}` : '';
          span.setAttribute('title', `作者: ${item.author_nickname}${note}`);
        }
      }
    });
  }

  // ---------- selection capture ----------
  function captureSelection() {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) return null;
    const range = sel.getRangeAt(0);
    const startBlock = ancestorWithBlockId(range.startContainer);
    const endBlock = ancestorWithBlockId(range.endContainer);
    if (!startBlock || startBlock !== endBlock) return null; // 跨块拒绝
    const blockId = startBlock.getAttribute('data-block-id');
    const start = textOffsetWithin(startBlock, range.startContainer, range.startOffset);
    const end = textOffsetWithin(startBlock, range.endContainer, range.endOffset);
    if (start == null || end == null || end <= start) return null;
    const blockText = startBlock.textContent || '';
    const exact = blockText.slice(start, end);
    const prefix = blockText.slice(Math.max(0, start - 32), start);
    const suffix = blockText.slice(end, Math.min(blockText.length, end + 32));
    const rect = range.getBoundingClientRect();
    return { block_id: blockId, start_offset: start, end_offset: end,
             exact_text: exact, prefix_text: prefix, suffix_text: suffix,
             rect: { top: rect.top, left: rect.left, width: rect.width, height: rect.height } };
  }

  function ancestorWithBlockId(node) {
    let n = node;
    while (n && n.nodeType !== 1) n = n.parentNode;
    while (n) { if (n.getAttribute && n.getAttribute('data-block-id')) return n; n = n.parentNode; }
    return null;
  }

  function textOffsetWithin(root, node, offsetInNode) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
    let acc = 0;
    while (walker.nextNode()) {
      const tn = walker.currentNode;
      if (tn === node) return acc + offsetInNode;
      acc += tn.nodeValue.length;
    }
    // 兼容：选区端点是元素而非文本节点
    if (node.nodeType === 1) {
      const walker2 = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, null);
      let acc2 = 0;
      while (walker2.nextNode()) {
        const tn = walker2.currentNode;
        if (node.contains(tn)) acc2 += tn.nodeValue.length;
      }
      return acc2;
    }
    return null;
  }

  // ---------- toolbar ----------
  let toolbar = null;
  // 最后一次选择的 visibility，跨选区记忆
  const VIS_PERSIST_KEY = 'rie-anno-last-visibility';
  function lastVisibility() {
    try { return localStorage.getItem(VIS_PERSIST_KEY) || 'private'; }
    catch (_) { return 'private'; }
  }
  function rememberVisibility(v) {
    try { localStorage.setItem(VIS_PERSIST_KEY, v); } catch (_) {}
  }
  function showToolbar(sel) {
    hideToolbar();
    toolbar = document.createElement('div');
    toolbar.className = 'rie-anno-toolbar';
    toolbar.style.top = (window.scrollY + sel.rect.top - 36) + 'px';
    toolbar.style.left = (window.scrollX + sel.rect.left) + 'px';
    // visibility 选择器
    const visSel = document.createElement('select');
    visSel.className = 'rie-vis-select';
    visSel.title = '可见范围';
    [
      ['private',       '🔒 私密'],
      ['course-public', '📚 本课程公开'],
      ['doc-public',    '📄 本文档公开'],
      ['public',        '🌐 全站公开'],
    ].forEach(([val, label]) => {
      const o = document.createElement('option');
      o.value = val; o.textContent = label;
      visSel.appendChild(o);
    });
    visSel.value = lastVisibility();
    visSel.addEventListener('change', () => rememberVisibility(visSel.value));
    // 避免点击 select 时被 mousedown 全局监听隐藏
    visSel.addEventListener('mousedown', (e) => e.stopPropagation());
    toolbar.appendChild(visSel);

    const colors = ['yellow','green','blue','pink','purple'];
    colors.forEach(c => {
      const b = document.createElement('button');
      b.className = `rie-swatch-${c}`;
      b.title = c;
      b.onclick = () => create(sel, c, visSel.value);
      toolbar.appendChild(b);
    });
    [['underline','U'], ['wavy','~'], ['strikethrough','S']].forEach(([s, label]) => {
      const b = document.createElement('button');
      b.className = `rie-style-${s === 'strikethrough' ? 'strike' : s}`;
      b.title = s; b.textContent = label;
      b.onclick = () => create(sel, s, visSel.value);
      toolbar.appendChild(b);
    });
    document.body.appendChild(toolbar);
  }
  function hideToolbar() { if (toolbar) { toolbar.remove(); toolbar = null; } }

  function create(sel, style, visibility) {
    hideToolbar();
    const ctx = window.RIE_ANNO_CTX || {};
    const vis = visibility || 'private';
    rememberVisibility(vis);
    const payload = {
      payload: {
        resource_kind: ctx.kind || 'course',
        resource_path: ctx.path || '',
        block_id: sel.block_id,
        start_offset: sel.start_offset,
        end_offset: sel.end_offset,
        exact_text: sel.exact_text,
        prefix_text: sel.prefix_text,
        suffix_text: sel.suffix_text,
        style: style,
        note: null,
        visibility: vis,
      },
    };
    fetch('/api/annotations/create', {
      method: 'POST',
      credentials: 'include',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(payload),
    }).then(r => r.ok ? r.json() : null).then(item => {
      if (!item) return;
      // 拼接到现有列表后再次 apply
      const data = window.__rieAnnoLast || { kind: ctx.kind, path: ctx.path, items: [] };
      data.items = (data.items || []).concat([item]);
      apply(data);
      window.__rieAnnoLast = data;
    }).catch(() => {});
  }

  // ---------- bootstrap ----------
  document.addEventListener('mouseup', (e) => {
    if (e.target && e.target.closest && e.target.closest('.rie-anno-toolbar')) return;
    setTimeout(() => {
      const sel = captureSelection();
      if (sel) showToolbar(sel); else hideToolbar();
    }, 10);
  });
  document.addEventListener('mousedown', (e) => {
    if (e.target && e.target.closest && e.target.closest('.rie-anno-toolbar')) return;
    hideToolbar();
  });

  // ---------- visibility toggle ----------
  const VIS_KEY = 'rie-anno-visible';
  function isVisible() {
    try { return localStorage.getItem(VIS_KEY) !== '0'; }
    catch (_) { return true; }
  }
  function setVisible(v) {
    try { localStorage.setItem(VIS_KEY, v ? '1' : '0'); } catch (_) {}
    if (v) document.body.classList.remove('no-anno');
    else document.body.classList.add('no-anno');
  }
  function toggleVisible() {
    const next = !isVisible();
    setVisible(next);
    return next;
  }
  // 初始同步 body class。document.body 可能在脚本加载时尚不存在，延迟一下。
  function applyInitialVisibility() {
    if (document.body) setVisible(isVisible());
    else document.addEventListener('DOMContentLoaded', () => setVisible(isVisible()), { once: true });
  }

  // ---------- hash flash (从个人标注列表跳回原文时闪烁目标块) ----------
  function flashTargetFromHash() {
    const raw = (location.hash || '').replace(/^#/, '');
    if (!raw) return;
    const el = findBlock(raw) || document.getElementById(raw);
    if (!el) return;
    try { el.scrollIntoView({ behavior: 'smooth', block: 'center' }); } catch (_) {}
    el.classList.remove('rie-anno-flash');
    // 重启动画
    void el.offsetWidth;
    el.classList.add('rie-anno-flash');
    setTimeout(() => el.classList.remove('rie-anno-flash'), 4000);
  }
  // 首次加载、路由 hash 变化、apply 完成后都重试一次
  function tryFlashLater() { setTimeout(flashTargetFromHash, 50); }
  window.addEventListener('hashchange', tryFlashLater);

  ensureStyles();
  applyInitialVisibility();
  window.RIE_ANNO = {
    apply: function (data) {
      window.RIE_ANNO_CTX = { kind: data.kind, path: data.path };
      window.__rieAnnoLast = data;
      apply(data);
      // 标注渲染完后试着闪烁一下（此时 DOM 类名可能已包裹，闪烁背景更明显）
      tryFlashLater();
    },
    captureSelection: captureSelection,
    isVisible: isVisible,
    setVisible: setVisible,
    toggleVisible: toggleVisible,
    flashTargetFromHash: flashTargetFromHash,
  };
  // 如果打开页面时已带 hash（所有 anno 资源准备好之前也允许先闪一下原生 block）
  if (location.hash) tryFlashLater();

  // 处理之前存进 pending 的数据
  if (window.__rieAnnoPending) {
    window.RIE_ANNO.apply(window.__rieAnnoPending);
    window.__rieAnnoPending = null;
  }
})();
