(function () {
    'use strict';

    // Dérive l'URL du manifest depuis l'emplacement du script :
    //   script : {base}/javascripts/downloads-manifest.js
    //   cible  : {base}/telechargements/manifest.json
    // Gère correctement les déploiements sous sous-chemin (ex: /projet/…).
    var scriptSrc = (document.currentScript && document.currentScript.src) || '';
    var MANIFEST_URL = scriptSrc
        ? new URL('../telechargements/manifest.json', scriptSrc).href
        : '/telechargements/manifest.json';

    // Base pour construire les href des versions antérieures (= {base}/telechargements/)
    var TELECHARGEMENTS_BASE = scriptSrc
        ? new URL('../telechargements/', scriptSrc).href
        : '/telechargements/';

    // Accepte n'importe quel nom de fichier .img dans latest/ (ex: IGN-BDTOPO-FRANCE-SE.img)
    var HREF_RE = /\/telechargements\/files\/([^/]+)\/([^/]+)\/latest\/[^/]+\.img$/;

    function formatBytes(bytes) {
        if (!bytes || bytes <= 0) return '';
        var units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
        var i = 0;
        var v = bytes;
        while (v >= 1024 && i < units.length - 1) {
            v /= 1024;
            i += 1;
        }
        return v.toFixed(v < 10 && i > 0 ? 1 : 0) + ' ' + units[i];
    }

    function formatDate(iso) {
        if (!iso) return '';
        var d = new Date(iso);
        if (isNaN(d.getTime())) return '';
        try {
            return d.toLocaleDateString('fr-FR', { year: 'numeric', month: 'long', day: 'numeric' });
        } catch (e) {
            return iso.slice(0, 10);
        }
    }

    function buildMetaBlock(version) {
        if (!version || !version.published_at) return null;

        var wrapper = document.createElement('div');
        wrapper.className = 'download-meta';

        var date = document.createElement('span');
        date.className = 'download-meta-date';
        date.textContent = '📅 ' + formatDate(version.published_at);
        wrapper.appendChild(date);

        return wrapper;
    }

    function enhanceLink(link, manifest) {
        // link.href = URL absolue résolue par le browser (support href relatifs type ../files/...).
        var href = link.href || link.getAttribute('href') || '';
        var m = href.match(HREF_RE);
        if (!m) return;
        var key = m[1] + '/' + m[2];
        var entry = manifest.coverages && manifest.coverages[key];

        if (!entry) {
            link.classList.add('is-unavailable');
            link.textContent = 'Non disponible';
            link.setAttribute('aria-disabled', 'true');
            link.addEventListener('click', function (e) { e.preventDefault(); });
            return;
        }

        link.classList.add('is-available');

        // Source de vérité = entry.latest (calculé côté serveur)
        var allVersions = entry.versions || [];
        var latestVersion = entry.latest;
        var latest = null;
        var older = [];
        allVersions.forEach(function (v) {
            if (v.version === latestVersion) latest = v;
            else older.push(v);
        });
        // Tri des versions antérieures par published_at décroissant (fallback version desc)
        older.sort(function (a, b) {
            var ka = a.published_at || a.version;
            var kb = b.published_at || b.version;
            return ka < kb ? 1 : (ka > kb ? -1 : 0);
        });

        if (latest) {
            var size = formatBytes(latest.size_bytes);
            var label = 'Télécharger';
            if (size) label += ' (' + size + ' — ' + latestVersion + ')';
            else label += ' (' + latestVersion + ')';
            link.textContent = label;

            // Bloc méta sous le bouton : date de publication + sha256 tronqué copiable
            var meta = buildMetaBlock(latest);
            if (meta && link.parentNode) {
                if (link.nextSibling) {
                    link.parentNode.insertBefore(meta, link.nextSibling);
                } else {
                    link.parentNode.appendChild(meta);
                }
            }
        }

        if (older.length === 0) return;

        var details = document.createElement('details');
        details.className = 'downloads-previous-versions';
        var summary = document.createElement('summary');
        summary.textContent = 'Versions antérieures';
        details.appendChild(summary);
        var ul = document.createElement('ul');
        older.forEach(function (v) {
            var li = document.createElement('li');
            var a = document.createElement('a');
            a.href = TELECHARGEMENTS_BASE + v.path;
            var sz = formatBytes(v.size_bytes);
            a.textContent = v.version + (sz ? ' — ' + sz : '');
            li.appendChild(a);
            ul.appendChild(li);
        });
        details.appendChild(ul);

        if (link.parentNode) {
            if (link.nextSibling) {
                link.parentNode.insertBefore(details, link.nextSibling);
            } else {
                link.parentNode.appendChild(details);
            }
        }
    }

    function run() {
        // Sélecteur tolérant aux href relatifs (ex: ../files/...) — on filtre ensuite
        // sur link.href (URL absolue résolue par le browser).
        var links = Array.prototype.filter.call(
            document.querySelectorAll('a[href*="files/"][href*="/latest/"][href$=".img"]'),
            function (a) { return HREF_RE.test(a.href || a.getAttribute('href') || ''); }
        );
        if (links.length === 0) return;

        fetch(MANIFEST_URL, { cache: 'no-cache' })
            .then(function (r) {
                if (!r.ok) throw new Error('manifest fetch failed: ' + r.status);
                return r.json();
            })
            .then(function (manifest) {
                links.forEach(function (link) { enhanceLink(link, manifest); });
            })
            .catch(function () {
                // Fallback silencieux : les boutons restent fonctionnels vers latest/.
            });
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', run);
    } else {
        run();
    }
})();
