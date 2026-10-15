import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';
import starlight from '@astrojs/starlight';
import { readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import starlightThemeBlack from 'starlight-theme-black';
import rehypeExternalLinks from 'rehype-external-links';

/** After a build, open every off-site link in a new tab without exposing this
 *  page to it. Covers links Starlight and the theme render themselves (the
 *  GitHub link, social icons, "Edit page"); markdown links are already
 *  handled by rehype-external-links. */
const externalLinksInNewTab = () => ({
  name: 'external-links-in-new-tab',
  hooks: {
    'astro:build:done': async ({ dir }) => {
      const root = fileURLToPath(dir);
      const files = (await readdir(root, { recursive: true })).filter(
        (file) => file.endsWith('.html') && !file.startsWith(`demo${path.sep}`)
      );
      await Promise.all(
        files.map(async (file) => {
          const full = path.join(root, file);
          const html = await readFile(full, 'utf8');
          const updated = html.replace(/<a\b(?=[^>]*\bhref="https?:\/\/)(?![^>]*\btarget=)([^>]*)>/g, (_tag, attrs) => {
            const rel = /\brel="/.test(attrs)
              ? attrs.replace(/\brel="([^"]*)"/, (_match, value) => `rel="${value} noopener noreferrer"`)
              : `${attrs} rel="noopener noreferrer"`;
            return `<a${rel} target="_blank">`;
          });
          if (updated !== html) await writeFile(full, updated);
        })
      );
    }
  }
});

export default defineConfig({
  site: 'https://powermise.dev',
  markdown: {
    // External links in the docs open in a new tab, without giving the
    // destination access to this page.
    rehypePlugins: [[rehypeExternalLinks, { target: '_blank', rel: ['noopener', 'noreferrer'] }]]
  },
  vite: {
    build: {
      cssMinify: 'esbuild',
      chunkSizeWarningLimit: 500
    },
    resolve: {
      alias: {
        'starlight-theme-black/overrides/Sidebar.astro': path.resolve('src/starlight-theme-black/overrides/Sidebar.astro')
      }
    }
  },
  integrations: [
    externalLinksInNewTab(),
    sitemap(),
    starlight({
      title: 'powermise',
      description: 'An unofficial desktop companion app for mise-en-place.',
      // The same icon as the landing page (src/pages/index.astro).
      favicon: '/favicon.ico',
      disable404Route: true,
      customCss: ['./src/styles/docs.css'],
      plugins: [
        starlightThemeBlack({
          navLinks: [{ label: 'GitHub', link: 'https://github.com/frankievalentine/powermise' }],
          footerText: 'powermise v0.1.0'
        })
      ],
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/frankievalentine/powermise' }],
      editLink: {
        baseUrl: 'https://github.com/frankievalentine/powermise/edit/main/apps/web/'
      },
      lastUpdated: true,
      components: {
        Head: './src/components/DocHead.astro'
      },
      sidebar: [
        {
          label: 'Start here',
          items: ['what-is-powermise', 'getting-started', 'app-tour']
        },
        {
          label: 'How it works',
          items: ['how-it-works', 'troubleshooting']
        },
        {
          label: 'AI Setups',
          items: [
            'ai-setups',
            'ai-setups/terminal',
            'ai-setups/harness-selections',
            'ai-setups/selection-schema',
            'ai-setups/catalog',
            'ai-setups/harness-reference',
            'ai-setups/targets'
          ]
        }
      ]
    })
  ]
});
