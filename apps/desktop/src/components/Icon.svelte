<script lang="ts">
  let { name, size = 22 }: { name: string; size?: number } = $props();
  const paths: Record<string, string> = {
    home: 'm3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-8H9v8H4a1 1 0 0 1-1-1Z',
    user: 'M20 21v-2a7 7 0 0 0-14 0v2M16 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0',
    environment: 'M3 6c0-4 18-4 18 0s-18 4-18 0m0 0v12c0 4 18 4 18 0V6M3 12c0 4 18 4 18 0',
    activity: 'M7 3H4v18h16V3h-3M8 2h8v4H8ZM8 11h8M8 15h8',
    settings: 'm9 3-1 3-3 1-2 3 2 2-1 3 3 2 2-1 3 2 3-2 2 1 3-2-1-3 2-2-2-3-3-1-1-3Zm6 9a3 3 0 1 1-6 0 3 3 0 0 1 6 0',
    grid: 'M3 3h7v7H3ZM14 3h7v7h-7ZM3 14h7v7H3ZM14 14h7v7h-7Z',
    arrow: 'M4 12h15m-6-6 6 6-6 6',
    switch: 'M4 7h15m-4-4 4 4-4 4M20 17H5m4-4-4 4 4 4',
    globe: 'M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0M3 12h18M12 3a20 20 0 0 1 0 18 20 20 0 0 1 0-18M5 6h14M5 18h14',
    pin: 'M19 10c0 6-7 12-7 12S5 16 5 10a7 7 0 0 1 14 0m-4 0a3 3 0 1 1-6 0 3 3 0 0 1 6 0',
    laptop: 'M4 4h16v13H4ZM2 20h20l-2-3H4Z',
    server: 'M3 4h18v6H3ZM3 14h18v6H3ZM7 7h.01M7 17h.01M11 7h6M11 17h6',
    star: 'M12 2v20M2 12h20M5 5l14 14M5 19 19 5',
    check: 'm5 12 4 4L19 6',
    refresh: 'M20 7V2m0 5h-5M4 17v5m0-5h5M20 7a9 9 0 0 0-15-3M4 17a9 9 0 0 0 15 3',
    external: 'M14 3h7v7m0-7L10 14M10 3H3v18h18v-7',
    lock: 'M5 10h14v11H5ZM8 10V6a4 4 0 0 1 8 0v4M12 14v3',
    image: 'M3 3h18v18H3Zm0 14 6-6 5 5 3-3 4 4M17 7h.01',
    alert: 'M12 8v5m0 4h.01M12 2 1 22h22Z',
    pause: 'M8 4v16M16 4v16',
  };
</script>
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={paths[name] ?? paths.grid}/></svg>
