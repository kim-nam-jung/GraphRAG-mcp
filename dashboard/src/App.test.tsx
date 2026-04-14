import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import App from './App';

import { act } from '@testing-library/react';

describe('App Dashboard', () => {
  it('renders the title card properly', async () => {
    await act(async () => {
      render(<App />);
    });
    expect(screen.getByText('GraphRAG Topology')).toBeInTheDocument();
  });
  
  it('renders controls', async () => {
    await act(async () => {
      render(<App />);
    });
    expect(screen.getByText('Force Physics')).toBeInTheDocument();
    expect(screen.getByText('DAG Hierarchy')).toBeInTheDocument();
  });
});
