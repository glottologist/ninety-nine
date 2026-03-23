// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded affix "><a href="introduction.html">Introduction</a></li><li class="chapter-item expanded affix "><li class="part-title">User Guide</li><li class="chapter-item expanded "><a href="getting-started/installation.html"><strong aria-hidden="true">1.</strong> Getting Started</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="getting-started/configuration.html"><strong aria-hidden="true">1.1.</strong> Configuration</a></li></ol></li><li class="chapter-item expanded "><a href="guides/usage.html"><strong aria-hidden="true">2.</strong> Usage</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="guides/detecting.html"><strong aria-hidden="true">2.1.</strong> Running Tests</a></li><li class="chapter-item expanded "><a href="guides/interactive-tui.html"><strong aria-hidden="true">2.2.</strong> Interactive TUI</a></li><li class="chapter-item expanded "><a href="guides/filter-dsl.html"><strong aria-hidden="true">2.3.</strong> Filter DSL</a></li><li class="chapter-item expanded "><a href="guides/quarantine.html"><strong aria-hidden="true">2.4.</strong> Quarantine Management</a></li><li class="chapter-item expanded "><a href="guides/exporting.html"><strong aria-hidden="true">2.5.</strong> Exporting Results</a></li><li class="chapter-item expanded "><a href="guides/ci-integration.html"><strong aria-hidden="true">2.6.</strong> CI Integration</a></li></ol></li><li class="chapter-item expanded "><a href="guides/best-practices.html"><strong aria-hidden="true">3.</strong> Best Practices</a></li><li class="chapter-item expanded "><a href="guides/troubleshooting.html"><strong aria-hidden="true">4.</strong> Troubleshooting</a></li><li class="chapter-item expanded affix "><li class="part-title">API Reference</li><li class="chapter-item expanded "><a href="api/types.html"><strong aria-hidden="true">5.</strong> Types</a></li><li class="chapter-item expanded "><a href="api/detector.html"><strong aria-hidden="true">6.</strong> Bayesian Detector</a></li><li class="chapter-item expanded "><a href="api/analysis.html"><strong aria-hidden="true">7.</strong> Analysis</a></li><li class="chapter-item expanded "><a href="api/filter.html"><strong aria-hidden="true">8.</strong> Filter DSL</a></li><li class="chapter-item expanded "><a href="api/storage.html"><strong aria-hidden="true">9.</strong> Storage</a></li><li class="chapter-item expanded "><a href="api/runner.html"><strong aria-hidden="true">10.</strong> Runner</a></li><li class="chapter-item expanded "><a href="api/config.html"><strong aria-hidden="true">11.</strong> Configuration</a></li><li class="chapter-item expanded "><a href="api/errors.html"><strong aria-hidden="true">12.</strong> Error Types</a></li><li class="chapter-item expanded affix "><li class="part-title">Reference</li><li class="chapter-item expanded "><a href="reference/architecture.html"><strong aria-hidden="true">13.</strong> Architecture</a></li><li class="chapter-item expanded "><a href="reference/bayesian.html"><strong aria-hidden="true">14.</strong> Bayesian Detection</a></li><li class="chapter-item expanded "><a href="reference/analysis.html"><strong aria-hidden="true">15.</strong> Analysis &amp; Patterns</a></li><li class="chapter-item expanded "><a href="reference/storage.html"><strong aria-hidden="true">16.</strong> Storage</a></li><li class="chapter-item expanded "><a href="reference/cli.html"><strong aria-hidden="true">17.</strong> CLI Reference</a></li><li class="chapter-item expanded "><a href="reference/configuration.html"><strong aria-hidden="true">18.</strong> Configuration Reference</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0].split("?")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
