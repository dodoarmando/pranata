import type { JSONContent } from '@tiptap/core';
import { renderToHTMLString } from '@tiptap/static-renderer/pm/html-string';
import { richTextExtensions } from './rich-text';

/** A News Post body as HTML; server-only, so Tiptap never reaches a public page's bundle. */
export const renderRichText = (body: object) =>
  renderToHTMLString({ extensions: richTextExtensions, content: body as JSONContent });
