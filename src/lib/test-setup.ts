// Tests laufen auf Deutsch, unabhängig von der Sprache des Rechners.
import { vi } from 'vitest';

vi.stubGlobal('navigator', { language: 'de-DE' });
