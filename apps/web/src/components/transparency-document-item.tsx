import { Download, FileText } from 'lucide-react';
import type { PublishedTransparencyDocument } from '../api/generated';
import { documentTypeLabels, fileSize } from '../transparency-document';

/** A Published Transparency Document in a public list, with its download link. */
export function TransparencyDocumentItem({
  document,
  heading: Heading = 'h2',
}: {
  document: PublishedTransparencyDocument;
  /** Its title's level, under the page's or section's own heading. */
  heading?: 'h2' | 'h3';
}) {
  return (
    <li className="flex items-center gap-4 rounded-md border bg-card p-4">
      <span className="grid size-10 flex-none place-items-center rounded-md bg-muted text-primary">
        <FileText className="size-5" aria-hidden="true" />
      </span>
      <div className="grid min-w-0 flex-1 gap-1">
        <Heading className="leading-snug font-semibold">{document.title}</Heading>
        <p className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
          <span className="rounded-full border px-2 py-0.5 text-xs font-medium text-foreground">
            {documentTypeLabels[document.documentType]}
          </span>
          Tahun anggaran {document.fiscalYear} ·{' '}
          <span className="whitespace-nowrap">PDF · {fileSize(document.byteSize)}</span>
        </p>
      </div>
      <a
        href={`/media/${document.file}/pdf`}
        download={`${document.title}.pdf`}
        aria-label={`Unduh ${document.title}`}
        className="inline-flex h-9 flex-none items-center gap-1.5 rounded-md border px-3 text-sm font-medium hover:border-primary hover:text-primary"
      >
        <span className="max-sm:sr-only">Unduh</span>
        <Download className="size-4" aria-hidden="true" />
      </a>
    </li>
  );
}
