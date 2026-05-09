(function () {
    'use strict';

    // Détection de la langue depuis l'attribut lang de <html> (positionné par Zensical).
    var lang = (document.documentElement.lang || 'fr').toLowerCase();
    var isEn = lang === 'en';

    // Table i18n — toutes les chaînes visibles par l'utilisateur.
    var T = isEn ? {
        unavailable:      'Not available',
        download:         'Download',
        copy:             'Copy',
        copy_label:       'Copy command',
        copied:           'Copied!',
        close_label:      'Close',
        older_versions:   'Previous versions',
        commands_btn:     '⚙ Build commands',
        modal_prefix:     'Commands — ',
        tab_labels:       ['Download', 'Compilation', 'mpforge', 'imgforge'],
        sec_download:     'BDTOPO data download',
        sec_compile:      'Map compilation (build-garmin-map.sh)',
        sec_mpforge:      'Tiling (mpforge)',
        sec_imgforge:     'IMG compilation (imgforge)',
        date_locale:      'en-GB',
        env_comment:      '# Environment variables required by mpforge (YAML config)',
    } : {
        unavailable:      'Non disponible',
        download:         'Télécharger',
        copy:             'Copier',
        copy_label:       'Copier la commande',
        copied:           'Copié !',
        close_label:      'Fermer',
        older_versions:   'Versions antérieures',
        commands_btn:     '⚙ Commandes de compilation',
        modal_prefix:     'Commandes — ',
        tab_labels:       ['Téléchargement', 'Compilation', 'mpforge', 'imgforge'],
        sec_download:     'Téléchargement des données BDTOPO',
        sec_compile:      'Compilation de la carte (build-garmin-map.sh)',
        sec_mpforge:      'Tuilage (mpforge)',
        sec_imgforge:     'Compilation IMG (imgforge)',
        date_locale:      'fr-FR',
        env_comment:      '# Variables d\'environnement requises par mpforge (config YAML)',
    };

    // Dérive l'URL du manifest depuis l'emplacement du script :
    //   script : {base}/javascripts/downloads-manifest.js
    //   cible  : {base}/telechargements/manifest.json
    // Pour la version EN (/en/javascripts/…), on remonte à la racine du site
    // car le manifest est toujours publié sous /telechargements/.
    var scriptSrc = (document.currentScript && document.currentScript.src) || '';
    var MANIFEST_URL, TELECHARGEMENTS_BASE;
    if (isEn) {
        // Racine du site = origin (manifest toujours sous /telechargements/)
        var origin = window.location.origin || '';
        MANIFEST_URL       = origin + '/telechargements/manifest.json';
        TELECHARGEMENTS_BASE = origin + '/telechargements/';
    } else {
        MANIFEST_URL = scriptSrc
            ? new URL('../telechargements/manifest.json', scriptSrc).href
            : '/telechargements/manifest.json';
        TELECHARGEMENTS_BASE = scriptSrc
            ? new URL('../telechargements/', scriptSrc).href
            : '/telechargements/';
    }

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
    // Fallback générique : extrait type/slug depuis toute URL versionnée
    // Structure : .../<type>/<slug>/v<version>/<file>.img (S3 ou chemin local versionné)
    var HREF_RE_GENERIC = /\/([^/]+)\/([^/]+)\/v[^/]+\/[^/]+\.img$/;
    function matchGenericHref(href) {
        var m = href.match(HREF_RE_GENERIC);
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
            return d.toLocaleDateString(T.date_locale, { year: 'numeric', month: 'long', day: 'numeric' });
        } catch (e) {
            return iso.slice(0, 10);
        }
    }

    // Construit la commande download-bdtopo.sh depuis les build_params.
    function buildDownloadCmd(bp) {
        return [
            './scripts/download-data.sh \\',
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
        var lines = ['./scripts/build-garmin-map.sh \\'];
        function add(flag, val) {
            lines.push('    ' + flag + (val !== undefined ? ' ' + val : '') + ' \\');
        }

        add('--zones', bp.zones);
        add('--base-id', bp.base_id);
        add('--year', bp.year);
        add('--version', bp.version);

        add('--data-dir', './pipeline/data');
        add('--contours-dir', './pipeline/data/contours');
        add('--dem-dir', './pipeline/data/dem');
        add('--osm-dir', './pipeline/data/osm');
        add('--hiking-trails-dir', './pipeline/data/hiking-trails');
        add('--output-base', './pipeline/output');

        add('--jobs', bp.jobs || '8');
        if (bp.mpforge_jobs)  add('--mpforge-jobs', bp.mpforge_jobs);
        if (bp.imgforge_jobs) add('--imgforge-jobs', bp.imgforge_jobs);

        add('--family-id', bp.family_id);
        add('--product-id', bp.product_id || '1');
        add('--family-name', '"' + bp.family_name + '"');
        add('--series-name', '"' + (bp.series_name || 'IGN-BDTOPO-MAP') + '"');

        add('--code-page', bp.code_page || '1252');
        add('--levels', '"' + (bp.levels || '24,22,20,18,16') + '"');
        add('--typ', bp.typ_file || 'pipeline/resources/typfiles/I2023100.typ');

        add('--copyright', '"' + bp.copyright + '"');

        if (bp.packaging && bp.packaging !== 'legacy') add('--packaging', bp.packaging);

        if (bp.reduce_point_density)  add('--reduce-point-density', bp.reduce_point_density);
        if (bp.simplify_polygons)     add('--simplify-polygons', '"' + bp.simplify_polygons + '"');
        if (bp.min_size_polygon)      add('--min-size-polygon', bp.min_size_polygon);
        if (bp.merge_lines === 'true') add('--merge-lines');

        if (bp.with_dem   === 'false') add('--no-dem');
        if (bp.with_route === 'false' && bp.with_net !== 'true') add('--no-route');
        if (bp.with_net   === 'true')  add('--net');

        if (bp.draw_priority)                      add('--draw-priority', bp.draw_priority);
        if (bp.transparent            === 'true')  add('--transparent');
        if (bp.order_by_decreasing_area === 'true') add('--order-by-decreasing-area');
        if (bp.keep_going             === 'true')  add('--keep-going');

        if (bp.no_round_coords          === 'true') add('--no-round-coords');
        if (bp.no_size_filter           === 'true') add('--no-size-filter');
        if (bp.no_remove_obsolete_points === 'true') add('--no-remove-obsolete-points');

        if (bp.dem_dists)         add('--dem-dists', '"' + bp.dem_dists + '"');
        if (bp.dem_interpolation) add('--dem-interpolation', bp.dem_interpolation);

        add('--publish');
        add('--publish-target', bp.publish_target || 's3');
        add('-v');

        lines[lines.length - 1] = lines[lines.length - 1].replace(' \\', '');
        return lines.join('\n');
    }

    // Dérive le chemin du sources.yaml mpforge depuis l'entrée coverage.
    function getConfigPath(entry) {
        var type = entry.type || '';
        var slug = entry.slug || '';
        if (type === 'quadrant') {
            return 'pipeline/configs/ign-bdtopo/france-quadrant/sources.yaml';
        }
        var outreMerSlugs = {
            'd971': 'la-guadeloupe',
            'd972': 'la-martinique',
            'd973': 'la-guyane',
            'd974': 'la-reunion',
            'd976': 'mayotte'
        };
        if (outreMerSlugs[slug]) {
            return 'pipeline/configs/ign-bdtopo/outre-mer/' + outreMerSlugs[slug] + '/sources.yaml';
        }
        return 'pipeline/configs/ign-bdtopo/departement/sources.yaml';
    }

    // Construit la commande mpforge build depuis les build_params.
    function buildMpforgeCmd(bp, entry) {
        var config = getConfigPath(entry);
        var jobs = bp.mpforge_jobs || bp.jobs || '8';
        var zonesLabel = (entry.slug || bp.zones || 'ZONES').toUpperCase();
        var outputDir = './pipeline/output/' + bp.year + '/' + bp.version + '/' + zonesLabel;
        return [
            T.env_comment,
            'export DATA_ROOT=./pipeline/data',
            'export OUTPUT_DIR=' + outputDir,
            'export BASE_ID=' + bp.base_id,
            'export ZONES=' + bp.zones,
            '',
            'mpforge build \\',
            '    --config ' + config + ' \\',
            '    --report ' + outputDir + '/mpforge-report.json \\',
            '    --jobs ' + jobs
        ].join('\n');
    }

    // Construit la commande imgforge build depuis les build_params.
    function buildImgforgeCmd(bp, entry) {
        var zonesLabel = (entry.slug || bp.zones || 'ZONES').toUpperCase();
        var outputDir = './pipeline/output/' + bp.year + '/' + bp.version + '/' + zonesLabel;
        var mpDir = outputDir + '/mp';
        var outImg = outputDir + '/img/' + bp.family_name + '.img';
        var jobs = bp.imgforge_jobs || bp.jobs || '8';

        var lines = ['imgforge build ' + mpDir + ' \\'];
        function add(flag, val) {
            lines.push('    ' + flag + (val !== undefined ? ' ' + val : '') + ' \\');
        }

        add('--output', outImg);
        add('--report', outputDir + '/imgforge-report.json');
        add('--jobs', jobs);
        add('--family-id', bp.family_id);
        add('--product-id', bp.product_id || '1');
        add('--family-name', '"' + bp.family_name + '"');
        add('--series-name', '"' + (bp.series_name || 'IGN-BDTOPO-MAP') + '"');
        add('--code-page', bp.code_page || '1252');
        add('--lower-case');
        add('--levels', '"' + (bp.levels || '24,22,20,18,16') + '"');
        add('--copyright-message', '"' + bp.copyright + '"');

        if (bp.with_net === 'true') {
            add('--net');
        } else if (bp.with_route !== 'false') {
            add('--route');
        } else {
            add('--no-route');
        }

        if (bp.typ_file) add('--typ-file', bp.typ_file);
        add('--packaging', bp.packaging || 'legacy');

        if (bp.reduce_point_density)  add('--reduce-point-density', bp.reduce_point_density);
        if (bp.simplify_polygons)     add('--simplify-polygons', '"' + bp.simplify_polygons + '"');
        if (bp.min_size_polygon)      add('--min-size-polygon', bp.min_size_polygon);
        if (bp.merge_lines === 'true') add('--merge-lines');

        if (bp.with_dem === 'true') {
            var depts = (bp.zones || '').split(',');
            depts.forEach(function (dept) {
                var d = dept.trim();
                if (d) add('--dem', './pipeline/data/dem/' + d);
            });
            add('--dem-source-srs', '"EPSG:2154"');
            if (bp.dem_dists)         add('--dem-dists', '"' + bp.dem_dists + '"');
            if (bp.dem_interpolation) add('--dem-interpolation', bp.dem_interpolation);
        }

        if (bp.keep_going               === 'true') add('--keep-going');
        if (bp.order_by_decreasing_area === 'true') add('--order-by-decreasing-area');
        if (bp.draw_priority)                       add('--draw-priority', bp.draw_priority);
        if (bp.transparent              === 'true') add('--transparent');
        if (bp.no_round_coords          === 'true') add('--no-round-coords');
        if (bp.no_size_filter           === 'true') add('--no-size-filter');
        if (bp.no_remove_obsolete_points === 'true') add('--no-remove-obsolete-points');

        lines[lines.length - 1] = lines[lines.length - 1].replace(' \\', '');
        return lines.join('\n');
    }

    // Copie via execCommand (fallback clipboard).
    function execCommandCopy(text, done) {
        var ta = document.createElement('textarea');
        ta.value = text;
        ta.style.position = 'fixed';
        ta.style.opacity = '0';
        document.body.appendChild(ta);
        ta.select();
        try { document.execCommand('copy'); done(); } catch (e) {}
        document.body.removeChild(ta);
    }

    // Crée un bouton "Copier" associé à un texte donné.
    function createCopyBtn(text) {
        var btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'dl-modal-copy';
        btn.textContent = T.copy;
        btn.setAttribute('aria-label', T.copy_label);
        btn.addEventListener('click', function () {
            var done = function () {
                btn.textContent = T.copied;
                btn.classList.add('is-copied');
                setTimeout(function () {
                    btn.textContent = T.copy;
                    btn.classList.remove('is-copied');
                }, 1500);
            };
            if (navigator.clipboard && navigator.clipboard.writeText) {
                navigator.clipboard.writeText(text).then(done, function () {
                    execCommandCopy(text, done);
                });
            } else {
                execCommandCopy(text, done);
            }
        });
        return btn;
    }

    // Infrastructure modale (création paresseuse, instance unique).
    var _modal = null;
    var _modalTrigger = null;

    function getOrCreateModal() {
        if (_modal) return _modal;

        var overlay = document.createElement('div');
        overlay.className = 'dl-modal-overlay';
        overlay.setAttribute('role', 'dialog');
        overlay.setAttribute('aria-modal', 'true');
        overlay.setAttribute('aria-hidden', 'true');
        overlay.setAttribute('aria-labelledby', 'dl-modal-title');

        var dialog = document.createElement('div');
        dialog.className = 'dl-modal-dialog';

        var header = document.createElement('div');
        header.className = 'dl-modal-header';

        var title = document.createElement('h3');
        title.className = 'dl-modal-title';
        title.id = 'dl-modal-title';
        header.appendChild(title);

        var closeBtn = document.createElement('button');
        closeBtn.type = 'button';
        closeBtn.className = 'dl-modal-close';
        closeBtn.setAttribute('aria-label', T.close_label);
        closeBtn.textContent = '×';
        closeBtn.addEventListener('click', function () { closeModal(); });
        header.appendChild(closeBtn);

        var body = document.createElement('div');
        body.className = 'dl-modal-body';

        dialog.appendChild(header);
        dialog.appendChild(body);
        overlay.appendChild(dialog);
        document.body.appendChild(overlay);

        overlay.addEventListener('click', function (e) {
            if (e.target === overlay) closeModal();
        });

        document.addEventListener('keydown', function (e) {
            if (!_modal || !_modal.overlay.classList.contains('is-open')) return;
            if (e.key === 'Escape') {
                closeModal();
                return;
            }
            if (e.key === 'Tab') {
                var focusable = Array.prototype.slice.call(
                    dialog.querySelectorAll('button:not([disabled]), [href], input:not([disabled]), [tabindex]:not([tabindex="-1"])')
                );
                if (focusable.length === 0) return;
                var first = focusable[0];
                var last = focusable[focusable.length - 1];
                if (e.shiftKey) {
                    if (document.activeElement === first) { e.preventDefault(); last.focus(); }
                } else {
                    if (document.activeElement === last) { e.preventDefault(); first.focus(); }
                }
            }
        });

        _modal = { overlay: overlay, title: title, body: body, closeBtn: closeBtn };
        return _modal;
    }

    function closeModal() {
        if (!_modal) return;
        _modal.overlay.classList.remove('is-open');
        _modal.overlay.setAttribute('aria-hidden', 'true');
        if (_modalTrigger) { _modalTrigger.focus(); _modalTrigger = null; }
    }

    function activateTab(idx, tabBtns, panels) {
        tabBtns.forEach(function (t, i) {
            var active = i === idx;
            t.classList.toggle('is-active', active);
            t.setAttribute('aria-selected', active ? 'true' : 'false');
            t.tabIndex = active ? 0 : -1;
            panels[i].hidden = !active;
        });
    }

    function openModal(titleText, sections, triggerEl) {
        var m = getOrCreateModal();
        _modalTrigger = triggerEl || null;
        m.title.textContent = titleText;

        while (m.body.firstChild) m.body.removeChild(m.body.firstChild);

        var tablist = document.createElement('div');
        tablist.className = 'dl-modal-tablist';
        tablist.setAttribute('role', 'tablist');
        m.body.appendChild(tablist);

        var tabBtns = [];
        var panels = [];

        sections.forEach(function (pair, i) {
            var tabId   = 'dl-tab-btn-' + i;
            var panelId = 'dl-tab-panel-' + i;
            var shortLabel = T.tab_labels[i] || pair[0];

            var tab = document.createElement('button');
            tab.type = 'button';
            tab.className = 'dl-modal-tab' + (i === 0 ? ' is-active' : '');
            tab.setAttribute('role', 'tab');
            tab.setAttribute('aria-selected', i === 0 ? 'true' : 'false');
            tab.setAttribute('aria-controls', panelId);
            tab.id = tabId;
            tab.tabIndex = i === 0 ? 0 : -1;
            tab.textContent = shortLabel;
            tablist.appendChild(tab);
            tabBtns.push(tab);

            var panel = document.createElement('div');
            panel.className = 'dl-modal-panel';
            panel.setAttribute('role', 'tabpanel');
            panel.id = panelId;
            panel.setAttribute('aria-labelledby', tabId);
            if (i !== 0) panel.hidden = true;

            var wrapper = document.createElement('div');
            wrapper.className = 'dl-modal-pre-wrapper';
            var pre  = document.createElement('pre');
            var code = document.createElement('code');
            code.className = 'language-bash';
            code.textContent = pair[1];
            pre.appendChild(code);
            wrapper.appendChild(pre);
            wrapper.appendChild(createCopyBtn(pair[1]));
            panel.appendChild(wrapper);
            m.body.appendChild(panel);
            panels.push(panel);
        });

        tabBtns.forEach(function (tab, i) {
            tab.addEventListener('click', function () {
                activateTab(i, tabBtns, panels);
            });
        });

        tablist.addEventListener('keydown', function (e) {
            var idx = tabBtns.indexOf(document.activeElement);
            if (idx === -1) return;
            var next = idx;
            if      (e.key === 'ArrowRight') next = (idx + 1) % tabBtns.length;
            else if (e.key === 'ArrowLeft')  next = (idx - 1 + tabBtns.length) % tabBtns.length;
            else if (e.key === 'Home')        next = 0;
            else if (e.key === 'End')         next = tabBtns.length - 1;
            else return;
            e.preventDefault();
            activateTab(next, tabBtns, panels);
            tabBtns[next].focus();
        });

        m.overlay.classList.add('is-open');
        m.overlay.setAttribute('aria-hidden', 'false');
        m.closeBtn.focus();
    }

    // Construit le bouton déclencheur de la modal des commandes.
    function buildCommandsTrigger(bp, entry) {
        if (!bp || !bp.zones || !bp.version) return null;

        var label = (entry && entry.label) ? entry.label : (bp.zones + ' ' + bp.version);
        var sections = [
            [T.sec_download, buildDownloadCmd(bp)],
            [T.sec_compile,  buildCompileCmd(bp)],
            [T.sec_mpforge,  buildMpforgeCmd(bp, entry || {})],
            [T.sec_imgforge, buildImgforgeCmd(bp, entry || {})]
        ];

        var btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'dl-cmds-trigger';
        btn.setAttribute('aria-haspopup', 'dialog');
        btn.textContent = T.commands_btn;
        btn.addEventListener('click', function () {
            openModal(T.modal_prefix + label, sections, btn);
        });
        return btn;
    }

    function buildMetaBlock(version) {
        if (!version || !version.published_at) return null;
        var formatted = formatDate(version.published_at);
        if (!formatted) return null;

        var wrapper = document.createElement('div');
        wrapper.className = 'download-meta';

        var date = document.createElement('span');
        date.className = 'download-meta-date';
        date.textContent = '📅 ' + formatted;
        wrapper.appendChild(date);

        return wrapper;
    }

    function enhanceLink(link, manifest) {
        var href = link.href || link.getAttribute('href') || '';
        var key = null;
        var isConfidentMatch = false;
        var m = href.match(HREF_RE_LOCAL);
        if (m) {
            key = m[1] + '/' + m[2];
            isConfidentMatch = true;
        } else {
            var endpoint = manifest.storage && manifest.storage.endpoint_public;
            var s3 = matchS3Href(href, endpoint);
            if (s3) { key = s3.type + '/' + s3.slug; isConfidentMatch = true; }
        }
        if (!key) {
            var generic = matchGenericHref(href);
            if (generic) key = generic.type + '/' + generic.slug;
        }
        if (!key) return;
        var entry = manifest.coverages && manifest.coverages[key];

        if (!entry) {
            if (!isConfidentMatch) return;
            link.classList.add('is-unavailable');
            link.textContent = T.unavailable;
            link.setAttribute('aria-disabled', 'true');
            link.addEventListener('click', function (e) { e.preventDefault(); });
            return;
        }

        link.classList.add('is-available');

        var allVersions = entry.versions || [];
        var latestVersion = entry.latest;
        var latest = null;
        var older = [];
        allVersions.forEach(function (v) {
            if (v.version === latestVersion) latest = v;
            else older.push(v);
        });
        older.sort(function (a, b) {
            var ka = a.published_at || a.version;
            var kb = b.published_at || b.version;
            return ka < kb ? 1 : (ka > kb ? -1 : 0);
        });

        if (latest) {
            var size = formatBytes(latest.size_bytes);
            var label = T.download;
            if (size) label += ' (' + size + ' — ' + latestVersion + ')';
            else label += ' (' + latestVersion + ')';
            link.textContent = label;

            var meta = buildMetaBlock(latest);
            if (meta && link.parentNode) {
                if (link.nextSibling) {
                    link.parentNode.insertBefore(meta, link.nextSibling);
                } else {
                    link.parentNode.appendChild(meta);
                }
            }

            var trigger = buildCommandsTrigger(latest.build_params, entry);
            if (trigger && link.parentNode) {
                var anchor = meta || link;
                if (anchor.nextSibling) {
                    link.parentNode.insertBefore(trigger, anchor.nextSibling);
                } else {
                    link.parentNode.appendChild(trigger);
                }
            }
        }

        if (older.length === 0) return;

        var details = document.createElement('details');
        details.className = 'downloads-previous-versions';
        var summary = document.createElement('summary');
        summary.textContent = T.older_versions;
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
