import { useCallback, useState } from "react";
import { STORAGE_KEY_DENSITY } from "./constants";

export type Density = "large" | "normal" | "compact" | "dense";

export const DENSITIES: { value: Density; label: string; hint: string }[] = [
  { value: "large", label: "Large", hint: "Bigger text and more room" },
  { value: "normal", label: "Normal", hint: "The default" },
  { value: "compact", label: "Compact", hint: "More on screen" },
  { value: "dense", label: "Dense", hint: "As much as fits" },
];

function isDensity(value: unknown): value is Density {
  return DENSITIES.some((d) => d.value === value);
}

/** The saved display size, or `normal` when none is saved or storage is unavailable. */
export function storedDensity(): Density {
  try {
    const value = localStorage.getItem(STORAGE_KEY_DENSITY);
    return isDensity(value) ? value : "normal";
  } catch {
    return "normal";
  }
}

/** Sizes the interface; `index.css` maps each value to a root font size. */
export function applyDensity(density: Density): void {
  document.documentElement.dataset.density = density;
}

export function useDensity(): [Density, (density: Density) => void] {
  const [density, setState] = useState<Density>(storedDensity);
  const setDensity = useCallback((next: Density) => {
    setState(next);
    applyDensity(next);
    try {
      localStorage.setItem(STORAGE_KEY_DENSITY, next);
    } catch {
      // Applied for this visit; not remembered.
    }
  }, []);
  return [density, setDensity];
}
