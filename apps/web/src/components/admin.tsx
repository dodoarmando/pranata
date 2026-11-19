import { CircleAlert, Info } from 'lucide-react';
import type { ReactNode } from 'react';
import { Alert, AlertDescription } from './ui/alert';
import { Table } from './ui/table';

/** An admin page's title and its one-line description. */
export function PageHead({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div>
      <h1 className="text-xl font-semibold">{title}</h1>
      {children && <p className="text-muted-foreground">{children}</p>}
    </div>
  );
}

/** The outcome of an action, or why a page cannot be shown. */
export function Notice({ error = false, children }: { error?: boolean; children: ReactNode }) {
  return (
    <Alert variant={error ? 'destructive' : 'default'}>
      {error ? <CircleAlert aria-hidden="true" /> : <Info aria-hidden="true" />}
      <AlertDescription className={error ? undefined : 'text-foreground'}>
        {children}
      </AlertDescription>
    </Alert>
  );
}

/** A bordered table; the Table inside scrolls sideways rather than squeezing its columns. */
export function DataTable({ children }: { children: ReactNode }) {
  return (
    <div className="overflow-hidden rounded-lg border [&_thead]:bg-muted">
      <Table>{children}</Table>
    </div>
  );
}
