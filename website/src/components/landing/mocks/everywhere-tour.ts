import { CLI_STEP_MS } from "./everywhere-terminal";

/** The four ways to open HarnessKit, in the order the tour shows them; the radios' values in EverywherePanel. */
export const MODES = ["mac", "win", "web", "cli"] as const;
export type Mode = (typeof MODES)[number];
/** How long each window stays on before the next, its progress line filling meanwhile. */
export const STEP_MS = 2400;
/** Each way's time on: the terminal's longer, while its session types itself out (everywhere-terminal.ts). */
export const stepFor = (mode: Mode) => (mode === "cli" ? CLI_STEP_MS : STEP_MS);
/** How much of the picture must be on-screen before the tour plays. */
export const TOUR_RATIO = 0.5;
// What counts as the visitor taking over.
const TOUCHES = ["click", "keydown", "focusin", "change"] as const;

/**
 * Plays the tour on a loop: the picture starts on the first way (the page's inline script checked it) and moves on
 * once that way's time is up (stepFor), from the last back to the first. It plays while TOUR_RATIO of `root` is on-screen and the tab is
 * visible, and otherwise pauses where it is, the way in view and its progress held, to go on from there. A mouse
 * resting on the picture doesn't pause it (WCAG 2.2.2 is met by the click that stops it). The visitor's first click,
 * key or focus inside `root` ends it where it is (a click, not a press: a finger that lands on the picture to scroll
 * the page leaves it alone). Setting `checked` fires no change event, so the tour never passes for the visitor. `root` carries
 * data-tour="playing" or "paused" while the tour lasts (the switch draws the progress line from it), "waiting" before
 * it first plays, and none once it ends. Returns a function that ends it.
 */
export function tour(root: HTMLElement): () => void {
  const show = (mode: Mode) => {
    const input = root.querySelector<HTMLInputElement>(`input[value="${mode}"]`);
    if (input) input.checked = true;
  };
  let index = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  // The step's time still to run, and when it last started running.
  let left = stepFor(MODES[0]);
  let since = 0;
  let stage: "waiting" | "playing" | "paused" | "over" = "waiting";
  let inView = false;

  const run = () => {
    since = Date.now();
    timer = setTimeout(() => {
      index = (index + 1) % MODES.length;
      show(MODES[index]);
      left = stepFor(MODES[index]);
      run();
    }, left);
  };
  const update = () => {
    if (stage === "over") return;
    const free = inView && !document.hidden;
    if (free && stage !== "playing") {
      stage = "playing";
      root.dataset.tour = "playing";
      run();
    } else if (!free && stage === "playing") {
      clearTimeout(timer);
      left = Math.max(0, left - (Date.now() - since));
      stage = "paused";
      root.dataset.tour = "paused";
    }
  };
  const end = () => {
    stage = "over";
    clearTimeout(timer);
    io.disconnect();
    for (const type of TOUCHES) root.removeEventListener(type, end);
    document.removeEventListener("visibilitychange", update);
    delete root.dataset.tour;
  };

  const io = new IntersectionObserver((entries) => {
    inView = entries[entries.length - 1].intersectionRatio >= TOUR_RATIO;
    update();
  }, { threshold: [0, TOUR_RATIO] });
  io.observe(root);
  for (const type of TOUCHES) root.addEventListener(type, end);
  document.addEventListener("visibilitychange", update);
  return end;
}
