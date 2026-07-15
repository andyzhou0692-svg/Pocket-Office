import type { APIRoute } from 'astro';

// robots.txt, prerendered to dist/robots.txt. The sitemap URL is DERIVED from
// the one canonical origin (`site` in astro.config.mjs) + BASE_URL, so a
// later Pocket Office domain decision cannot leave a stale hardcoded origin.
export const GET: APIRoute = ({ site }) => {
  const base = import.meta.env.BASE_URL.replace(/\/$/, '');
  const sitemap = new URL(`${base}/sitemap-index.xml`, site);
  return new Response(`User-agent: *\nAllow: /\n\nSitemap: ${sitemap.href}\n`, {
    headers: { 'Content-Type': 'text/plain; charset=utf-8' },
  });
};
