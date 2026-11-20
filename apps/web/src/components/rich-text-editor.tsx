import type { JSONContent } from '@tiptap/core';
import { EditorContent, useEditor, useEditorState } from '@tiptap/react';
import {
  Bold,
  Columns3,
  Heading2,
  Heading3,
  Italic,
  Link,
  List,
  ListOrdered,
  Pilcrow,
  Quote,
  Rows3,
  SquarePlay,
  Table,
  Trash2,
} from 'lucide-react';
import type { ReactNode } from 'react';
import { richTextExtensions, youtubeId } from '../rich-text';

/** The admin area's rich-text field: only what the API's allowlist accepts. */
export function RichTextEditor({
  content,
  onChange,
  labelledBy,
}: {
  content: object;
  onChange: (content: JSONContent) => void;
  labelledBy: string;
}) {
  const editor = useEditor({
    extensions: richTextExtensions,
    content: content as JSONContent,
    // SSR renders nothing; the editor mounts in the browser.
    immediatelyRender: false,
    editorProps: {
      attributes: {
        class: 'rich-text min-h-64 px-3 py-2 outline-none',
        role: 'textbox',
        'aria-multiline': 'true',
        'aria-labelledby': labelledBy,
      },
    },
    onUpdate: ({ editor }) => onChange(editor.getJSON()),
  });
  const active = useEditorState({
    editor,
    selector: ({ editor }) => ({
      paragraph: editor?.isActive('paragraph') ?? false,
      h2: editor?.isActive('heading', { level: 2 }) ?? false,
      h3: editor?.isActive('heading', { level: 3 }) ?? false,
      bold: editor?.isActive('bold') ?? false,
      italic: editor?.isActive('italic') ?? false,
      link: editor?.isActive('link') ?? false,
      bulletList: editor?.isActive('bulletList') ?? false,
      orderedList: editor?.isActive('orderedList') ?? false,
      blockquote: editor?.isActive('blockquote') ?? false,
      table: editor?.isActive('table') ?? false,
    }),
  });

  const chain = () => editor?.chain().focus();

  function onLink() {
    const current = editor?.getAttributes('link').href ?? '';
    const input = window.prompt(
      'Alamat tautan (https://, mailto: atau tel:); kosongkan untuk menghapus',
      current,
    );
    if (input === null) return;
    const href = input.trim();
    if (!href) {
      chain()?.extendMarkRange('link').unsetLink().run();
      return;
    }
    const full = /^[a-z]+:/i.test(href) ? href : `https://${href}`;
    if (!/^(https?:\/\/|mailto:|tel:)./i.test(full)) {
      window.alert('Tautan harus diawali https://, http://, mailto: atau tel:.');
      return;
    }
    chain()?.extendMarkRange('link').setLink({ href: full }).run();
  }

  function onYoutube() {
    const input = window.prompt('Alamat video YouTube');
    if (!input) return;
    const videoId = youtubeId(input);
    if (!videoId) {
      window.alert('Itu bukan alamat video YouTube.');
      return;
    }
    chain()?.insertContent({ type: 'youtube', attrs: { videoId } }).run();
  }

  return (
    <div className="rounded-lg border border-input focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50">
      <div role="toolbar" aria-label="Format teks" className="flex flex-wrap gap-0.5 border-b p-1">
        <Tool
          label="Paragraf"
          pressed={active?.paragraph}
          onClick={() => chain()?.setParagraph().run()}
        >
          <Pilcrow />
        </Tool>
        <Tool
          label="Judul bagian"
          pressed={active?.h2}
          onClick={() => chain()?.toggleHeading({ level: 2 }).run()}
        >
          <Heading2 />
        </Tool>
        <Tool
          label="Subjudul"
          pressed={active?.h3}
          onClick={() => chain()?.toggleHeading({ level: 3 }).run()}
        >
          <Heading3 />
        </Tool>
        <Tool label="Tebal" pressed={active?.bold} onClick={() => chain()?.toggleBold().run()}>
          <Bold />
        </Tool>
        <Tool label="Miring" pressed={active?.italic} onClick={() => chain()?.toggleItalic().run()}>
          <Italic />
        </Tool>
        <Tool label="Tautan" pressed={active?.link} onClick={onLink}>
          <Link />
        </Tool>
        <Tool
          label="Daftar"
          pressed={active?.bulletList}
          onClick={() => chain()?.toggleBulletList().run()}
        >
          <List />
        </Tool>
        <Tool
          label="Daftar bernomor"
          pressed={active?.orderedList}
          onClick={() => chain()?.toggleOrderedList().run()}
        >
          <ListOrdered />
        </Tool>
        <Tool
          label="Kutipan"
          pressed={active?.blockquote}
          onClick={() => chain()?.toggleBlockquote().run()}
        >
          <Quote />
        </Tool>
        <Tool
          label="Sisipkan tabel"
          onClick={() => chain()?.insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()}
        >
          <Table />
        </Tool>
        <Tool label="Sisipkan video YouTube" onClick={onYoutube}>
          <SquarePlay />
        </Tool>
        {active?.table && (
          <>
            <Tool label="Tambah baris" onClick={() => chain()?.addRowAfter().run()}>
              <Rows3 />
            </Tool>
            <Tool label="Tambah kolom" onClick={() => chain()?.addColumnAfter().run()}>
              <Columns3 />
            </Tool>
            <Tool label="Hapus baris" onClick={() => chain()?.deleteRow().run()}>
              <Rows3 className="text-destructive" />
            </Tool>
            <Tool label="Hapus kolom" onClick={() => chain()?.deleteColumn().run()}>
              <Columns3 className="text-destructive" />
            </Tool>
            <Tool label="Hapus tabel" onClick={() => chain()?.deleteTable().run()}>
              <Trash2 className="text-destructive" />
            </Tool>
          </>
        )}
      </div>
      <EditorContent editor={editor} />
    </div>
  );
}

function Tool({
  label,
  pressed,
  onClick,
  children,
}: {
  label: string;
  pressed?: boolean | undefined;
  onClick: () => unknown;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={pressed}
      onClick={onClick}
      className="grid size-8 place-items-center rounded-md hover:bg-accent aria-pressed:bg-accent aria-pressed:text-primary [&_svg]:size-4"
    >
      {children}
    </button>
  );
}
