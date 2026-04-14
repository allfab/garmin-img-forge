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

    // Lien local (legacy) : .../telechargements/files/<type>/<slug>/latest/*.img
    var HREF_RE_LOCAL = /\/telechargements\/files\/([^/]+)\/([^/]+)\/latest\/[^/]+\.img$/;
    // Lien S3 absolu : <endpoint_public>/<type>/<slug>/<version>/*.img
    // L'endpoint_public est lu depuis manifest.storage.endpoint_public (mode s3).
    function matchS3Href(href, endpoint) {
        if (!endpoint) return null;
        var prefix = endpoint.replace(/\/+$/, '') + '/';
        if (href.indexOf(prefix) !== 0) return null;
        var tail = href.slice(prefix.length);
        // type/slug/version/file.img
        var m = tail.match(/^([^/]+)\/([^/]+)\/[^/]+\/[^/]+\.img$/);
        return m ? { type: m[1], slug: m[2] } : null;
    }

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

    // Construit la commande download-bdtopo.sh depuis les build_params.
    function buildDownloadCmd(bp) {
        return [
            './scripts/download-bdtopo.sh \\',
            '    --zones ' + bp.zones + ' \\',
            '    --bdtopo-version ' + bp.version + ' \\',
            '    --format SHP \\',
            '    --with-contours \\',
            '    --with-osm \\',
            '    --with-dem \\',
            '    --dry-run'
        ].join('\n');
    }

    // Construit la commande build-garmin-map.sh depuis les build_params.
    function buildCompileCmd(bp) {
        return [
            './scripts/build-garmin-map.sh \\',
            '    --zones ' + bp.zones + ' \\',
            '    --base-id ' + bp.base_id + ' \\',
            '    --year ' + bp.year + ' \\',
            '    --version ' + bp.version + ' \\',
            '    --data-dir ./pipeline/data \\',
            '    --contours-dir ./pipeline/data/contours \\',
            '    --dem-dir ./pipeline/data/dem \\',
            '    --osm-dir ./pipeline/data/osm \\',
            '    --hiking-trails-dir ./pipeline/data/hiking-trails \\',
            '    --output-base ./pipeline/output \\',
            '    --config pipeline/configs/ign-bdtopo/sources.yaml \\',
            '    --jobs 8 \\',
            '    --family-id ' + bp.family_id + ' \\',
            '    --product-id 1 \\',
            '    --family-name "' + bp.family_name + '" \\',
            '    --series-name "IGN-BDTOPO-MAP" \\',
            '    --code-page 1252 \\',
            '    --levels "24,22,20,18,16" \\',
            '    --typ pipeline/resources/typfiles/I2023100.typ \\',
            '    --copyright "' + bp.copyright + '" \\',
            '    --publish \\',
            '    --publish-target s3 \\',
            '    -v'
        ].join('\n');
    }

    function buildCommandsBlock(bp) {
        if (!bp || !bp.zones || !bp.version) return null;

        var details = document.createElement('details');
        details.className = 'downloads-commands';

        var summary = document.createElement('summary');
        summary.textContent = 'Commandes de téléchargement et compilation';
        details.appendChild(summary);

        [
            ['Téléchargement des données BDTOPO', buildDownloadCmd(bp)],
            ['Compilation de la carte', buildCompileCmd(bp)]
        ].forEach(function (pair) {
            var h = document.createElement('p');
            h.className = 'downloads-commands-label';
            h.textContent = pair[0];
            details.appendChild(h);

            var wrapper = document.createElement('div');
            wrapper.className = 'downloads-commands-pre';

            var pre = document.createElement('pre');
            var code = document.createElement('code');
            code.className = 'language-bash';
            code.textContent = pair[1];
            pre.appendChild(code);
            wrapper.appendChild(pre);

            var btn = document.createElement('button');
            btn.type = 'button';
            btn.className = 'downloads-commands-copy';
            btn.textContent = 'Copier';
            btn.setAttribute('aria-label', 'Copier la commande');
            btn.addEventListener('click', function () {
                var text = pair[1];
                var done = function () {
                    var prev = btn.textContent;
                    btn.textContent = 'Copié !';
                    btn.classList.add('is-copied');
                    setTimeout(function () {
                        btn.textContent = prev;
                        btn.classList.remove('is-copied');
                    }, 1500);
                };
                if (navigator.clipboard && navigator.clipboard.writeText) {
                    navigator.clipboard.writeText(text).then(done, function () {});
                } else {
                    // Fallback : textarea temporaire + execCommand('copy').
                    var ta = document.createElement('textarea');
                    ta.value = text;
                    ta.style.position = 'fixed';
                    ta.style.opacity = '0';
                    document.body.appendChild(ta);
                    ta.select();
                    try { document.execCommand('copy'); done(); } catch (e) {}
                    document.body.removeChild(ta);
                }
            });
            wrapper.appendChild(btn);

            details.appendChild(wrapper);
        });

        return details;
    }

    function buildMetaBlock(version) {
        if (!version || !version.published_at) return null;
        var formatted = formatDate(version.published_at);
        if (!formatted) return null;  // Date invalide : ne pas injecter "📅 " nu.

        var wrapper = document.createElement('div');
        wrapper.className = 'download-meta';

        var date = document.createElement('span');
        date.className = 'download-meta-date';
        date.textContent = '📅 ' + formatted;
        wrapper.appendChild(date);

        return wrapper;
    }

    function enhanceLink(link, manifest) {
        // link.href = URL absolue résolue par le browser (support href relatifs type ../files/...).
        var href = link.href || link.getAttribute('href') || '';
        var key = null;
        var m = href.match(HREF_RE_LOCAL);
        if (m) {
            key = m[1] + '/' + m[2];
        } else {
            var endpoint = manifest.storage && manifest.storage.endpoint_public;
            var s3 = matchS3Href(href, endpoint);
            if (s3) key = s3.type + '/' + s3.slug;
        }
        if (!key) return;
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

            // Bloc repliable avec les commandes download + build pour cette version.
            var cmds = buildCommandsBlock(latest.build_params);
            if (cmds && link.parentNode) {
                var anchor = meta || link;
                if (anchor.nextSibling) {
                    link.parentNode.insertBefore(cmds, anchor.nextSibling);
                } else {
                    link.parentNode.appendChild(cmds);
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
        // On cible large (tout <a> vers .img) puis on filtre dans enhanceLink via
        // HREF_RE_LOCAL (legacy) ou matchS3Href (endpoint lu dans le manifest).
        var candidates = document.querySelectorAll('a[href$=".img"]');
        if (candidates.length === 0) return;

        fetch(MANIFEST_URL, { cache: 'no-cache' })
            .then(function (r) {
                if (!r.ok) throw new Error('manifest fetch failed: ' + r.status);
                return r.json();
            })
            .then(function (manifest) {
                Array.prototype.forEach.call(candidates, function (link) {
                    enhanceLink(link, manifest);
                });
            })
            .catch(function () {
                // Fallback silencieux : les boutons restent fonctionnels vers l'URL directe.
            });
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', run);
    } else {
        run();
    }
})();
