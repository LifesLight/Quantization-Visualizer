/**
 * Theme Management
 * Handles dark/light mode toggling, system preference detection, and the About modal.
 */
(function initializeThemeAndModals() {
    // 1. Initial Theme Setup
    const savedTheme = localStorage.getItem('theme');
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;

    if (savedTheme === 'dark' || (!savedTheme && prefersDark)) {
        document.documentElement.setAttribute('data-theme', 'dark');
    }

    document.addEventListener('DOMContentLoaded', () => {
        const themeToggle = document.getElementById('theme-toggle');
        const iconMoon = document.getElementById('theme-icon-moon');
        const iconSun = document.getElementById('theme-icon-sun');

        /**
         * Updates the visibility of the sun/moon icons based on current theme.
         */
        function updateThemeIcon() {
            const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
            if (iconMoon && iconSun) {
                iconMoon.style.display = isDark ? 'none' : 'block';
                iconSun.style.display = isDark ? 'block' : 'none';
            }
        }

        // 2. Theme Toggle Listeners
        if (themeToggle) {
            updateThemeIcon();

            themeToggle.addEventListener('click', () => {
                const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
                const newTheme = isDark ? 'light' : 'dark';

                document.documentElement.setAttribute('data-theme', newTheme);
                localStorage.setItem('theme', newTheme);
                updateThemeIcon();
            });

            // Listen for OS-level theme changes if no explicit override is set
            window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
                if (!localStorage.getItem('theme')) {
                    document.documentElement.setAttribute('data-theme', e.matches ? 'dark' : 'light');
                    updateThemeIcon();
                }
            });
        }

        // 3. About Modal Listeners
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

            // Close modal when clicking outside of it
            aboutModal.addEventListener('click', (e) => {
                if (e.target === aboutModal) {
                    aboutModal.style.display = 'none';
                }
            });
        }
    });
})();