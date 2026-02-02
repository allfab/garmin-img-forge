# Leçons Apprises - Implémentation OGR PolishMap Driver

## Story 1.4 - POI Layer Implementation with Feature Reading

### Problème 1 : Ordre d'Initialisation (Parser NULL)

**Symptôme** : `GetNextFeature()` retournait toujours `nullptr`. Le parser était NULL (`m_poParser=(nil)`).

**Cause Racine** :
- Le constructeur `OGRPolishMapDataSource()` appelait `CreateLayers()` immédiatement
- Mais le parser n'était défini que **plus tard** via `SetParser()` dans le driver
- Résultat : les couches étaient créées avec `m_poParser = nullptr`

**Solution** :
```cpp
// AVANT (INCORRECT)
OGRPolishMapDataSource::OGRPolishMapDataSource() {
    CreateLayers();  // ❌ Parser pas encore défini !
}

// APRÈS (CORRECT)
void OGRPolishMapDataSource::SetParser(std::unique_ptr<PolishMapParser> poParser) {
    m_poParser = std::move(poParser);
    CreateLayers();  // ✅ Parser défini maintenant
}
```

**Prévention Future** :
- ✅ Toujours initialiser les dépendances AVANT de créer les objets qui en dépendent
- ✅ Utiliser lazy initialization ou deux-phase initialization si nécessaire
- ✅ Ajouter des assertions/checks : `assert(m_poParser != nullptr)` avant utilisation

---

### Problème 2 : État Non-Initialisé du Parser

**Symptôme** : `ParseNextPOI()` n'était jamais appelé ou retournait immédiatement.

**Cause Racine** :
- Après `ParseHeader()`, le pointeur de fichier était à une position inconnue
- `GetNextFeature()` appelait `ParseNextPOI()` sans réinitialiser la position du fichier
- Le parser lisait depuis la mauvaise position ou EOF

**Solution** :
```cpp
// Sauvegarder la position après le header
m_nAfterHeaderPos = VSIFTellL(m_fpFile);

// Réinitialiser au premier appel de GetNextFeature()
if (!m_bReaderInitialized) {
    m_poParser->ResetPOIReading();  // Seek to m_nAfterHeaderPos
    m_bReaderInitialized = true;
}
```

**Prévention Future** :
- ✅ Toujours sauvegarder les positions de fichier importantes
- ✅ Implémenter `Reset()` pour réinitialiser l'état
- ✅ Utiliser des flags d'état (`m_bInitialized`) pour lazy initialization

---

### Problème 3 : Logique de Machine à États Incomplète

**Symptôme** : `ParseNextPOI()` retournait `nullptr` même avec des sections POI valides dans le fichier.

**Cause Racine** :
- Le parser rencontrait des sections non-POI (`[POLYLINE]`, `[POLYGON]`)
- Il détectait ces sections mais continuait à parser les lignes comme si c'était du contenu global
- Les données des sections non-POI polluaient ou bloquaient la lecture

**Solution** :
```cpp
bool bInPOISection = false;
bool bInOtherSection = false;  // ✅ Flag pour tracker les sections non-POI

while (ReadLine(osLine)) {
    if (osLine[0] == '[') {
        if (STARTS_WITH("[POI]")) {
            bInPOISection = true;
            bInOtherSection = false;
        } else if (STARTS_WITH("[END]")) {
            if (bInPOISection) return true;
            bInOtherSection = false;  // ✅ Reset après section non-POI
        } else {
            // Section non-POI détectée
            bInOtherSection = true;  // ✅ Activer le skip
        }
    }

    // ✅ Skip toutes les lignes dans les sections non-POI
    if (bInOtherSection) continue;

    if (bInPOISection) {
        // Parser les données POI
    }
}
```

**Prévention Future** :
- ✅ Dessiner un diagramme d'état AVANT d'implémenter un parser
- ✅ Gérer explicitement TOUS les états possibles
- ✅ Ajouter des flags pour tracker "ce qu'on NE veut PAS" (pas seulement ce qu'on veut)
- ✅ Tester avec des fichiers contenant des sections mixtes/imbriquées

---

### Problème 4 : Données de Test Inappropriées

**Symptôme** : Test d'encodage CP1252→UTF-8 échouait systématiquement.

**Cause Racine** :
- Le fichier de test `.mp` contenait déjà des caractères UTF-8 (`Café`, `Hôtel`)
- Le code essayait de les reconvertir depuis CP1252
- Les vrais fichiers Polish Map contiennent des **bytes bruts CP1252**, pas UTF-8

**Solution Temporaire** :
- Accepter l'échec du test pour Story 1.4
- Documenter que c'est un problème de données de test, pas de code

**Solution Future** :
- Créer des fichiers de test avec de vrais bytes CP1252 (hexedit ou écriture binaire)
- Ou : détecter si le fichier est déjà UTF-8 et skip la conversion

**Prévention Future** :
- ✅ Créer des données de test qui reflètent la VRAIE réalité du format
- ✅ Pour les tests d'encodage, utiliser des outils binaires (`xxd`, `hexedit`)
- ✅ Documenter clairement le format attendu des données de test

---

## Checklist pour Stories Futures

### Avant de Coder
- [ ] Vérifier l'ordre d'initialisation des dépendances
- [ ] Dessiner le diagramme d'états pour les parsers/machines à états
- [ ] Identifier tous les états possibles (y compris états "à ignorer")

### Pendant le Développement
- [ ] Ajouter des assertions sur les préconditions (`assert(ptr != nullptr)`)
- [ ] Implémenter `Reset()` pour tout objet avec état interne
- [ ] Utiliser lazy initialization avec flags si l'ordre d'init est complexe

### Tests
- [ ] Créer des données de test qui reflètent le format RÉEL
- [ ] Tester avec des données mixtes/désordonnées (pas seulement le "happy path")
- [ ] Vérifier que `Reset()` fonctionne correctement (tests d'itération multiple)

### Débogage
- [ ] Ajouter du logging CPLDebug() temporaire pour tracer l'exécution
- [ ] Vérifier les positions de fichier avec VSIFTellL()
- [ ] Utiliser un débogueur ou des printf pour inspecter l'état

---

## Pattern de Résolution Typique

1. **Symptôme** : Observer le comportement (NULL, pas de données, etc.)
2. **Isolation** : Ajouter du logging pour identifier où ça casse
3. **Analyse** : Identifier la cause racine (ordre d'init, état, logique)
4. **Fix** : Implémenter la solution minimale
5. **Vérification** : Retirer le logging, vérifier que les tests passent
6. **Documentation** : Ajouter à ce fichier !

---

## Métrique Story 1.4

- **Tests** : 5/6 passent (83%)
- **Bugs majeurs résolus** : 3
- **Temps de débogage** : ~60% du temps total
- **Cause principale** : Ordre d'initialisation + Machine à états incomplète

---

## Refactoring Post-Epic 1 - Élimination Duplication DRY

### Contexte

**Date** : 2026-02-02
**Owner** : Charlie + Elena (pair programming)
**Trigger** : Rétrospective Epic 1

### Problème Identifié

Les méthodes `ParseNextPOI()`, `ParseNextPolyline()`, et `ParseNextPolygon()` partageaient ~95% de code identique :

| Méthode | Lignes | Structure |
|---------|--------|-----------|
| ParseNextPOI() | 94 lignes | State machine + key/value parsing |
| ParseNextPolyline() | 112 lignes | Structure identique |
| ParseNextPolygon() | 112 lignes | Structure identique |
| **Total dupliqué** | **~300 lignes** | |

**Risques** :
- Chaque bug devait être corrigé à 3 endroits
- L'Epic 2 (Writer) aurait doublé cette dette avec `WritePOI()`, `WritePolyline()`, `WritePolygon()`

### Solution Implémentée

**1. Structure IR unifiée** (`polishmapparser.h`) :
```cpp
enum class SectionType { POI, Polyline, Polygon };

struct PolishMapSection {
    SectionType eType;
    std::string osType;
    std::string osLabel;
    std::vector<std::pair<double, double>> aoCoords;
    int nEndLevel;
    std::string osLevels;
    std::map<std::string, std::string> aoOtherFields;

    int GetMinPointCount() const;      // 1, 2, ou 3 selon eType
    const char* GetSectionMarker() const;  // "[POI]", "[POLYLINE]", "[POLYGON]"
    const char* GetTypeName() const;
};
```

**2. Méthode de parsing unifiée** (`polishmapparser.cpp`) :
```cpp
bool ParseNextSection(SectionType eTargetType, PolishMapSection& oSection);
```

**3. Wrappers pour rétrocompatibilité API** :
```cpp
bool ParseNextPOI(PolishMapPOISection& oSection) {
    PolishMapSection oGeneric(SectionType::POI);
    if (!ParseNextSection(SectionType::POI, oGeneric)) return false;
    // Conversion PolishMapSection -> PolishMapPOISection
    return true;
}
```

**4. Reset unifié** :
```cpp
void ResetSectionReading();  // Méthode principale
void ResetPOIReading() { ResetSectionReading(); }     // Alias inline
void ResetPolylineReading() { ResetSectionReading(); } // Alias inline
void ResetPolygonReading() { ResetSectionReading(); }  // Alias inline
```

### Résultats

| Métrique | Avant | Après | Réduction |
|----------|-------|-------|-----------|
| Lignes polishmapparser.cpp | 794 | 644 | **-150 lignes (19%)** |
| Méthodes de parsing | 3 dupliquées | 1 unifiée + 3 wrappers | Single point of fix |
| Tests | 72+ | 72+ | **100% pass** |
| API publique | Intacte | Intacte | Rétrocompatibilité préservée |

### Leçons Clés

1. **Refactorer AVANT d'ajouter de la dette** : Le refactoring fait avant Epic 2 évite de doubler la dette
2. **Wrappers pour rétrocompatibilité** : Préserver l'API publique via thin wrappers
3. **Tests comme filet de sécurité** : 72+ tests garantissent que le refactoring ne casse rien
4. **Structures unifiées avec helpers** : `GetMinPointCount()`, `GetTypeName()` encapsulent les différences

### Pattern Réutilisable pour Epic 2

Le même pattern peut être utilisé pour le Writer :
```cpp
bool WriteSection(const PolishMapSection& oSection);  // Méthode unifiée
bool WritePOI(const PolishMapPOISection& oSection);   // Wrapper
bool WritePolyline(const PolishMapPolylineSection& oSection);  // Wrapper
bool WritePolygon(const PolishMapPolygonSection& oSection);    // Wrapper
```

---

*Document mis à jour : 2026-02-02*
*Epic 1 complet, prêt pour Epic 2 !*
