# Integrar DESIGN.md como referencia Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dejar el sistema de diseño de Bóveda en `docs/DESIGN.md`, sin cambios, y enlazarlo desde los documentos que guían a quien trabaja en el repo.

**Architecture:** Solo documentación. Un archivo nuevo copiado tal cual y una línea de enlace en cada documento de guía; el diseño queda en un único lugar para no duplicar reglas.

**Tech Stack:** Markdown, git, `gh`.

**Spec:** diseño corto acordado en la conversación del 2026-11-19 (cambio pequeño, sin documento aparte). Alcance: solo referencia; no se crea código de tema, fuentes ni pantallas, y no se toca `docs/especificacion.md`.

## Global Constraints

- El archivo se llama `DESIGN.md` (mayúsculas, sin traducir) y va en `docs/`. Su contenido es idéntico al original en `C:\Users\jesus\Downloads\DESIGN.md`.
- Comentarios, documentación y mensajes de commit en español; Conventional Commits.
- Los `CLAUDE.md` solo enlazan al diseño; no copian reglas de él.
- No se modifica `docs/especificacion.md`: D1 a D6 de la sección 15 de `DESIGN.md` quedan pendientes y, mientras tanto, manda la especificación.
- La rama sale de `dev`; el PR va contra `dev` con el formato del PR #11 (Resumen, Decisiones de implementación, Criterios de aceptación con casillas, Verificación local).
- Los cambios de formato sin commit en `ARCHITECTURE.md` no entran en este PR.

## Review Focus

Plan solo de documentación: sin entradas que ejerciten pruebas. Lo que podría fallar son los enlaces rotos y que `DESIGN.md` no sea idéntico al original; la Task 1 lo comprueba con `cmp` y la Task 2 con `grep`.

---

### Task 1: Rama nueva y archivo de referencia

**Files:**
- Create: `docs/DESIGN.md`

**Interfaces:**
- Produces: `docs/DESIGN.md`, que la Task 2 enlaza.

- [ ] **Step 1: Apartar los cambios de formato de `ARCHITECTURE.md`**

Run: `git stash push -m "formato de tablas en ARCHITECTURE.md" -- ARCHITECTURE.md`
Expected: `Saved working directory and index state ...`; `git status --short` ya no lista `ARCHITECTURE.md`. El stash queda guardado para el usuario.

- [ ] **Step 2: Crear la rama desde `dev`**

Run: `git fetch origin && git switch -c docs/design-referencia origin/dev`
Expected: `Switched to a new branch 'docs/design-referencia'`. El plan, sin seguimiento, viaja con el cambio de rama. Si el usuario da un número de issue, la rama se llama `docs/<n>-design-referencia`.

- [ ] **Step 3: Copiar el archivo**

Copy `C:\Users\jesus\Downloads\DESIGN.md` to `docs/DESIGN.md` (herramienta Bash: `cp`).

- [ ] **Step 4: Comprobar que es idéntico**

Run: `cmp "/c/Users/jesus/Downloads/DESIGN.md" docs/DESIGN.md && echo IGUAL`
Expected: `IGUAL`

- [ ] **Step 5: Commit**

```bash
git add docs/DESIGN.md
git commit -m "docs: agregar el sistema de diseño como referencia"
```

### Task 2: Enlaces desde los documentos de guía

**Files:**
- Modify: `CLAUDE.md` (lista de arranque, tras la línea de `docs/especificacion.md`)
- Modify: `mobile/CLAUDE.md` (después del párrafo inicial)
- Modify: `desktop/CLAUDE.md` (después del párrafo inicial)
- Modify: `ARCHITECTURE.md` (línea "Documentos relacionados" y árbol de `docs/`)

**Interfaces:**
- Consumes: `docs/DESIGN.md` (Task 1).

- [ ] **Step 1: `CLAUDE.md` raíz.** Agregar este punto después del que menciona `docs/especificacion.md`: `- El diseño visual (colores, tipografía, componentes y pantallas) está en `docs/DESIGN.md`; léelo antes de crear o cambiar una pantalla, un widget o un estilo. Hasta que se decida lo contrario es solo referencia: si choca con `docs/especificacion.md`, manda la especificación.`

- [ ] **Step 2: `mobile/CLAUDE.md`.** Agregar, tras el párrafo inicial, la línea: `Antes de crear o cambiar una pantalla, un widget o un estilo, lee `docs/DESIGN.md` (secciones 2 a 7 y 9 a 13).`

- [ ] **Step 3: `desktop/CLAUDE.md`.** Agregar, tras el párrafo inicial: `Antes de crear o cambiar una ventana o un estilo, lee `docs/DESIGN.md` (secciones 2 a 4, 5.2 y 8).`

- [ ] **Step 4: `ARCHITECTURE.md`.** En "Documentos relacionados", sumar `` `docs/DESIGN.md` (cómo se ve) `` después de `docs/roadmap.md`. En el árbol de `docs/`, agregar `│   ├── DESIGN.md           # sistema de diseño de las dos apps (referencia)` entre `roadmap.md` y `superpowers/`, con los conectores `├──`/`└──` del árbol ajustados.

- [ ] **Step 5: Comprobar los enlaces**

Run: `grep -n "docs/DESIGN.md\|DESIGN.md" CLAUDE.md mobile/CLAUDE.md desktop/CLAUDE.md ARCHITECTURE.md`
Expected: una coincidencia en cada uno de los cuatro archivos (dos en `ARCHITECTURE.md`).

- [ ] **Step 6: Comprobar que no hay otros cambios**

Run: `git diff --stat`
Expected: solo esos cuatro archivos, con pocas líneas cambiadas; ningún realineado de tablas en `ARCHITECTURE.md`.

- [ ] **Step 7: Commit**

```bash
git add CLAUDE.md mobile/CLAUDE.md desktop/CLAUDE.md ARCHITECTURE.md
git commit -m "docs: enlazar el diseño desde CLAUDE.md y ARCHITECTURE.md"
```

### Task 3: Verificación y PR

- [ ] **Step 1: Pruebas y lint del repositorio** (regla del `CLAUDE.md` raíz, aunque el cambio sea solo documentación)

Run: `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`, cada uno con su código de salida visible (sin encadenar con `| grep`).
Expected: ambos terminan en 0.

- [ ] **Step 2: Publicar la rama** (pedir confirmación al usuario antes)

Run: `git push -u origin docs/design-referencia`

- [ ] **Step 3: Abrir el PR contra `dev`** con `gh pr create --base dev`, título `docs: agregar el sistema de diseño como referencia` y cuerpo con el formato del PR #11: Resumen, Decisiones de implementación (solo referencia, D1 a D6 sin registrar en la especificación, stash del formato de `ARCHITECTURE.md`), Criterios de aceptación con casillas y Verificación local.
Expected: URL del PR. No esperar la CI; dejar su casilla sin marcar.
