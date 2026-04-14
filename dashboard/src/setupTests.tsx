import '@testing-library/jest-dom';
import { vi } from 'vitest';

// Mock WebGL and ForceGraph3D for JSDOM
vi.mock('react-force-graph-3d', () => {
    return {
        default: () => <div data-testid="mock-graph-3d" />,
    };
});

// Mock fetch to prevent network errors in components
Object.defineProperty(globalThis, 'fetch', {
    value: vi.fn(() => 
        Promise.resolve({
            json: () => Promise.resolve({ entities: [], relations: [] })
        } as unknown as Response)
    )
});
