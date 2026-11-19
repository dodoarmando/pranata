import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/admin/')({
  component: Dashboard,
});

// Empty until the modules bring their dashboard cards.
function Dashboard() {
  return <h1>Beranda</h1>;
}
