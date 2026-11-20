import { createFileRoute } from '@tanstack/react-router';
import { publicOrigin } from '../api/fetch';
import { getVillage, listPublishedNewsPosts, type NewsCard } from '../api/generated';
import { rssXml } from '../feeds';
import { villageSentence } from '../seo';
import { villageTitle } from '../village';

const latest = 20;

export const Route = createFileRoute('/berita/rss.xml')({
  server: {
    handlers: {
      GET: async () => {
        const [village, first] = await Promise.all([getVillage(), listPublishedNewsPosts()]);
        if (village.status !== 200 || first.status !== 200) {
          return new Response(null, { status: 503 });
        }
        // The list comes a page at a time, newest first; fetch the pages the latest 20 span.
        const { pageSize, total } = first.data;
        const pages = Math.min(Math.ceil(latest / pageSize), Math.ceil(total / pageSize));
        const rest = await Promise.all(
          Array.from({ length: Math.max(0, pages - 1) }, (_, index) =>
            listPublishedNewsPosts({ page: index + 2 }),
          ),
        );
        const posts: NewsCard[] = [first, ...rest].flatMap((page) =>
          page.status === 200 ? page.data.posts : [],
        );
        const xml = rssXml(
          publicOrigin,
          villageTitle(village.data),
          villageSentence(village.data),
          posts.slice(0, latest),
        );
        return new Response(xml, {
          headers: { 'content-type': 'application/rss+xml; charset=utf-8' },
        });
      },
    },
  },
});
