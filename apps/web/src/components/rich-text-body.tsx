import type { MouseEvent } from 'react';

/** Turns a YouTube link in the body into a youtube-nocookie player, loading nothing before. */
function playYoutube(event: MouseEvent<HTMLElement>) {
  const link = (event.target as Element).closest<HTMLAnchorElement>('a[data-youtube]');
  if (!link?.dataset.youtube) return;
  event.preventDefault();
  const player = document.createElement('iframe');
  player.src = `https://www.youtube-nocookie.com/embed/${encodeURIComponent(link.dataset.youtube)}?autoplay=1`;
  player.title = 'Video YouTube';
  player.allow = 'autoplay; encrypted-media; picture-in-picture; fullscreen';
  player.className = 'aspect-video w-full rounded-md';
  link.replaceWith(player);
}

/** A rich-text body rendered to `html` on the server by rich-text-html. */
export function RichTextBody({ html }: { html: string }) {
  return (
    <>
      {/* biome-ignore lint/a11y/noStaticElementInteractions: delegates clicks on the YouTube links inside, which the keyboard reaches as links */}
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: see above */}
      <div
        className="rich-text"
        onClick={playYoutube}
        // biome-ignore lint/security/noDangerouslySetInnerHtml: rendered on the server from the API's allowlisted JSON, with every text escaped
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </>
  );
}
