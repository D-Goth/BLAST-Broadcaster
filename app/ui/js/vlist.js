// [UI-101]
const OVERSCAN = 8;

// [UI-102]
export function virtualList(container, rowHeight, renderRow, renderEmpty) {
  const spacer = document.createElement("div");
  spacer.className = "vspacer";
  container.replaceChildren(spacer);
  let count = 0;
  let pending = false;

  function draw() {
    pending = false;
    spacer.style.height = `${count * rowHeight}px`;
    if (count === 0) {
      spacer.replaceChildren(...(renderEmpty ? [renderEmpty()] : []));
      return;
    }
    const top = container.scrollTop;
    const height = container.clientHeight || 600;
    const first = Math.max(0, Math.floor(top / rowHeight) - OVERSCAN);
    const last = Math.min(count, Math.ceil((top + height) / rowHeight) + OVERSCAN);
    const frag = document.createDocumentFragment();
    for (let i = first; i < last; i++) {
      const el = renderRow(i);
      el.classList.add("vrow");
      el.style.top = `${i * rowHeight}px`;
      frag.appendChild(el);
    }
    spacer.replaceChildren(frag);
  }

  const schedule = () => {
    if (pending) return;
    pending = true;
    requestAnimationFrame(draw);
  };
  container.addEventListener("scroll", schedule, { passive: true });
  if (window.ResizeObserver) new ResizeObserver(schedule).observe(container);

  return {
    setCount(n) {
      count = n;
      schedule();
    },
    refresh: schedule,
  };
}
