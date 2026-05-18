(function () {
    const savedTheme = localStorage.getItem('theme');
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    if (savedTheme === 'dark' || (!savedTheme && prefersDark)) {
        document.documentElement.setAttribute('data-theme', 'dark');
    }

    document.addEventListener('DOMContentLoaded', () => {
        const themeToggle = document.getElementById('theme-toggle');
        const iconMoon = document.getElementById('theme-icon-moon');
        const iconSun = document.getElementById('theme-icon-sun');

        function updateThemeIcon() {
            const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
            if (iconMoon && iconSun) {
                iconMoon.style.display = isDark ? 'none' : 'block';
                iconSun.style.display = isDark ? 'block' : 'none';
            }
        }

        if (themeToggle) {
            updateThemeIcon();
            themeToggle.addEventListener('click', () => {
                const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
                document.documentElement.setAttribute('data-theme', isDark ? 'light' : 'dark');
                localStorage.setItem('theme', isDark ? 'light' : 'dark');
                updateThemeIcon();
            });

            window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
                if (!localStorage.getItem('theme')) {
                    document.documentElement.setAttribute('data-theme', e.matches ? 'dark' : 'light');
                    updateThemeIcon();
                }
            });
        }

        const btnAbout = document.getElementById('btn-about');
        const aboutModal = document.getElementById('about-modal');
        const btnAboutClose = document.getElementById('btn-about-close');

        if (btnAbout && aboutModal) {
            btnAbout.addEventListener('click', () => {
                aboutModal.style.display = 'flex';
            });
        }

        if (btnAboutClose && aboutModal) {
            btnAboutClose.addEventListener('click', () => {
                aboutModal.style.display = 'none';
            });
            aboutModal.addEventListener('click', (e) => {
                if (e.target === aboutModal) {
                    aboutModal.style.display = 'none';
                }
            });
        }
    });
})();