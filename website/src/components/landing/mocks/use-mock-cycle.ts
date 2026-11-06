import { useEffect, useRef, useState, type RefObject } from "react";

/** One beat of a mock animation: run `fn` this many milliseconds into the run. */
export type MockStep = readonly [ms: number, fn: () => void];

/** How much of the window must be on-screen before it plays. */
export const PLAY_RATIO = 0.5;

/**
 * Plays `steps` at their offsets and starts over every `period` ms, as the App's onboarding windows do; the 0 ms beats
 * reset the picture, so each run starts from the same first frame. The loop starts once PLAY_RATIO of `target` is
 * on-screen and the tab is visible. Once the window is fully off-screen it stops and rewinds (runs its 0 ms beats), so
 * the next visit starts from the first frame instead of watching the window jump back to it.
 * WCAG 2.2.2 (the motion starts on its own and lasts more than five seconds): the loop holds where it is while the
 * pointer is over the window or focus is inside it, and carries on from there after, so a reader can stop it to look.
 * It holds while the tab is hidden too. While it holds, `data-held` on the window lets CSS pause its animations.
 * Every timer is cleared on unmount.
 * Under prefers-reduced-motion nothing is scheduled; the return value turns true after mount so the caller
 * can show its finished state instead. It is false during the server render.
 */
export function useMockCycle(steps: readonly MockStep[], target: RefObject<HTMLElement | null>, period: number): boolean {
  const [reduced, setReduced] = useState(false);
  // Steps are usually written inline; a run always reads the latest ones.
  const stepsRef = useRef(steps);
  useEffect(() => { stepsRef.current = steps; });

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setReduced(true);
      return;
    }
    const el = target.current;
    let beats: readonly MockStep[] = [];
    let next = 0; // the first beat of the current run that hasn't fired
    let at = 0; // how far into the run it was when it last held
    let since = 0; // when it last started or carried on
    let timers: ReturnType<typeof setTimeout>[] = [];
    let looping = false; // between the first run's start and leaving the screen, held or not
    let inView = false;
    let hovered = false;
    let focused = false;

    const held = () => hovered || focused || document.hidden;
    const clear = () => {
      timers.forEach(clearTimeout);
      timers = [];
    };
    // Timers with ascending delays fire in order, so beat i firing means beats 0..i have run.
    const schedule = () => {
      since = Date.now();
      timers = beats.slice(next).map(([ms, fn]) => setTimeout(() => { next++; fn(); }, ms - at));
      timers.push(setTimeout(run, period - at));
    };
    const run = () => {
      clear();
      beats = [...stepsRef.current].sort((a, b) => a[0] - b[0]);
      next = 0;
      at = 0;
      schedule();
    };
    const update = () => {
      if (!looping) {
        if (inView && !held()) {
          looping = true;
          run();
        }
      } else if (held() && timers.length) {
        clear();
        at += Date.now() - since;
      } else if (!held() && !timers.length) {
        schedule();
      }
      el?.toggleAttribute("data-held", looping && held());
    };
    const rewind = () => {
      clear();
      looping = false;
      el?.removeAttribute("data-held");
      stepsRef.current.filter(([ms]) => ms === 0).forEach(([, fn]) => fn());
    };

    const io = el
      ? new IntersectionObserver((entries) => {
        const entry = entries[entries.length - 1];
        inView = entry.intersectionRatio >= PLAY_RATIO;
        if (entry.isIntersecting) update();
        else rewind();
      }, { threshold: [0, PLAY_RATIO] })
      : null;
    const onEnter = () => { hovered = true; update(); };
    const onLeave = () => { hovered = false; update(); };
    const onFocusIn = () => { focused = true; update(); };
    const onFocusOut = (e: FocusEvent) => {
      if (el?.contains(e.relatedTarget as Node | null)) return;
      focused = false;
      update();
    };
    if (el && io) {
      io.observe(el);
      el.addEventListener("pointerenter", onEnter);
      el.addEventListener("pointerleave", onLeave);
      el.addEventListener("focusin", onFocusIn);
      el.addEventListener("focusout", onFocusOut);
    } else inView = true;
    document.addEventListener("visibilitychange", update);
    update();
    return () => {
      clear();
      io?.disconnect();
      el?.removeEventListener("pointerenter", onEnter);
      el?.removeEventListener("pointerleave", onLeave);
      el?.removeEventListener("focusin", onFocusIn);
      el?.removeEventListener("focusout", onFocusOut);
      document.removeEventListener("visibilitychange", update);
    };
  }, [target, period]);

  return reduced;
}
