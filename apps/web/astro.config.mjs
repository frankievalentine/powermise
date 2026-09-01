import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';
import starlight from '@astrojs/starlight';
import path from 'node:path';
import starlightThemeBlack from 'starlight-theme-black';

export default defineConfig({
  site: 'https://powermise.dev',
  vite: {
    optimizeDeps: {
      include: ['three'],
    },
    build: {
      cssMinify: 'esbuild'
    },
    resolve: {
      alias: {
        'starlight-theme-black/overrides/Sidebar.astro': path.resolve('src/starlight-theme-black/overrides/Sidebar.astro')
      }
    }
  },
  integrations: [
    sitemap(),
    starlight({
      title: 'powermise',
      description: 'Curated AI-harness workstation configuration for mise.',
      favicon: './favicon.svg',
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
          items: ['what-is-powermise', 'getting-started']
        },
        {
          label: 'Reference',
          items: ['harness-selections', 'selection-schema', 'catalog', 'harness-reference', 'targets', 'commands']
        },
        {
          label: 'Guides',
          items: ['mise-boundary', 'lifecycle', 'shell-integration', 'automation', 'safety', 'coexistence', 'chezmoi', 'troubleshooting']
        }
      ]
    })
  ]
});
