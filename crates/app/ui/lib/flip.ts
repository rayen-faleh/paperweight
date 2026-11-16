// FLIP morphing for keyed elements that may be destroyed and re-created
// between renders (a slip moving to another column, or the whole view
// switching from Stacks to Near and far).
//
// Mark elements with data-flip="<stable id>". Call capture() before the DOM
// changes and morph() after. Elements that moved glide from where they were
// (including mid-animation, so retargeting is seamless); new ones fade up;
// ones that disappeared leave a ghost that fades away.

export interface Snapshot {
  rects: Map<string, DOMRect>;
  ghosts: Map<string, HTMLElement>;
}

export interface MorphOptions {
  /** Movement duration in ms. */
  duration?: number;
  /** Delay added per element, in DOM order, capped by maxStagger. */
  stagger?: number;
  maxStagger?: number;
}

// A soft ease-out with a hint of overshoot (about 2%), so moves land with a
// small settle instead of stopping dead. Small enough not to blur positions.
export const EASE_SETTLE = "cubic-bezier(.22, 1.12, .36, 1)";
export const EASE_OUT = "cubic-bezier(.22, .8, .24, 1)";

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

function flipEls(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>("[data-flip]"));
}

export function capture(root: HTMLElement): Snapshot {
  const rects = new Map<string, DOMRect>();
  const ghosts = new Map<string, HTMLElement>();
  const els = flipEls(root);
  // Measure everything first (mid-animation positions included), then stop
  // running animations so the next morph starts from what the eye saw.
  for (const el of els) {
    const id = el.dataset.flip!;
    if (el.style.opacity === "0") continue;
    rects.set(id, el.getBoundingClientRect());
    ghosts.set(id, el);
  }
  for (const el of els) el.getAnimations().forEach((a) => a.cancel());
  return { rects, ghosts };
}

function finalTransform(el: HTMLElement): string {
  const t = el.style.transform;
  return t && t !== "none" ? t : "";
}

export function morph(root: HTMLElement, before: Snapshot, opts: MorphOptions = {}): void {
  if (reduced()) return;
  const duration = opts.duration ?? 560;
  const stagger = opts.stagger ?? 14;
  const maxStagger = opts.maxStagger ?? 220;
  const seen = new Set<string>();
  flipEls(root).forEach((el, i) => {
    const id = el.dataset.flip!;
    if (el.style.opacity === "0") return;
    seen.add(id);
    const final = finalTransform(el);
    const delay = Math.min(i * stagger, maxStagger);
    const from = before.rects.get(id);
    const to = el.getBoundingClientRect();
    if (!from) {
      el.animate(
        [
          { opacity: 0, transform: `translateY(10px) ${final} scale(.97)` },
          { opacity: el.style.opacity || 1, transform: final || "none" },
        ],
        { duration: duration * 0.75, delay: delay + 90, easing: EASE_OUT, fill: "backwards" },
      );
      return;
    }
    const dx = from.left - to.left;
    const dy = from.top - to.top;
    if (Math.abs(dx) < 0.5 && Math.abs(dy) < 0.5) return;
    el.animate([{ transform: `translate(${dx}px, ${dy}px) ${final}` }, { transform: final || "none" }], {
      duration,
      delay,
      easing: EASE_SETTLE,
      fill: "backwards",
    });
  });
  // Ghosts: things that left fade out where they were.
  for (const [id, rect] of before.rects) {
    if (seen.has(id)) continue;
    const src = before.ghosts.get(id);
    if (!src || rect.width === 0) continue;
    const ghost = src.cloneNode(true) as HTMLElement;
    ghost.removeAttribute("data-flip");
    Object.assign(ghost.style, {
      position: "fixed",
      left: `${rect.left}px`,
      top: `${rect.top}px`,
      width: `${rect.width}px`,
      height: `${rect.height}px`,
      margin: "0",
      transform: "none",
      transition: "none",
      pointerEvents: "none",
      zIndex: "3",
      boxSizing: "border-box",
    });
    document.body.appendChild(ghost);
    ghost
      .animate(
        [
          { opacity: 1, transform: "none" },
          { opacity: 0, transform: "translateY(6px) scale(.96)" },
        ],
        { duration: 260, easing: EASE_OUT, fill: "forwards" },
      )
      .finished.finally(() => ghost.remove());
  }
}
