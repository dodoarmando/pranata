import { Node } from '@tiptap/core';
import { TableKit } from '@tiptap/extension-table';
import StarterKit from '@tiptap/starter-kit';

/** The YouTube video id in a watch, share, shorts or embed URL, or a bare id; null otherwise. */
export function youtubeId(input: string): string | null {
  const text = input.trim();
  if (/^[\w-]{11}$/.test(text)) return text;
  try {
    const url = new URL(text);
    const host = url.hostname.replace(/^(www|m)\./, '');
    const id =
      host === 'youtu.be'
        ? url.pathname.slice(1)
        : ['youtube.com', 'youtube-nocookie.com'].includes(host)
          ? (url.searchParams.get('v') ??
            url.pathname.match(/^\/(?:embed|shorts|live)\/([^/]+)/)?.[1])
          : null;
    return id && /^[\w-]{11}$/.test(id) ? id : null;
  } catch {
    return null;
  }
}

/**
 * A YouTube video kept as its id. It renders as a link to YouTube, which the public page turns
 * into a youtube-nocookie player on click, so nothing loads from YouTube before that.
 */
const Youtube = Node.create({
  name: 'youtube',
  group: 'block',
  atom: true,
  addAttributes: () => ({ videoId: { default: null } }),
  parseHTML: () => [
    { tag: 'a[data-youtube]', getAttrs: (element) => ({ videoId: element.dataset.youtube }) },
  ],
  renderHTML: ({ node }) => [
    'a',
    {
      href: `https://www.youtube.com/watch?v=${node.attrs.videoId}`,
      'data-youtube': node.attrs.videoId,
      class: 'youtube',
    },
    'Putar video YouTube',
  ],
});

/**
 * The editor's and the renderer's nodes and marks: exactly the API's allowlist (paragraph,
 * h2–h3, bold, italic, link, lists, blockquote, table, YouTube).
 */
export const richTextExtensions = [
  StarterKit.configure({
    heading: { levels: [2, 3] },
    code: false,
    codeBlock: false,
    hardBreak: false,
    horizontalRule: false,
    strike: false,
    underline: false,
    link: {
      openOnClick: false,
      defaultProtocol: 'https',
      isAllowedUri: (url) => /^(https?:\/\/|mailto:|tel:)/i.test(url),
    },
  }),
  TableKit.configure({ table: { resizable: false } }),
  Youtube,
];
