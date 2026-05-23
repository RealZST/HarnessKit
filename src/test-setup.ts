// Vitest global setup: initialize i18next once before any test imports a
// component that calls useTranslation. Without this, components throw because
// i18n resources are loaded synchronously via import.meta.glob at module-eval
// time, but tests that clear localStorage between cases can leave the detector
// in an inconsistent state.
import "@/lib/i18n";
import "@testing-library/jest-dom";
