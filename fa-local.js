document.addEventListener("DOMContentLoaded", () => {

  const processed = new Set();

  document.querySelectorAll("i[class*='fa-']").forEach(el => {

    // Cherche la classe fa-xxx (sauf fas, far, fab)
    const iconClass = [...el.classList].find(c =>
      c.startsWith("fa-") &&
      !["fa-solid","fa-regular","fa-brands","fas","far","fab"].includes(c)
    );

    if (!iconClass) return;

    const iconName = iconClass.replace("fa-", "");

    // Empêche de charger 10 fois la même icône
    if (processed.has(iconName)) return;
    processed.add(iconName);

    // Détecte le type (solid / regular / brands)
    let folder = "solid";
    if (el.classList.contains("far") || el.classList.contains("fa-regular")) folder = "regular";
    if (el.classList.contains("fab") || el.classList.contains("fa-brands")) folder = "brands";

    const url = `/img/${folder}/${iconName}.svg`;

    fetch(url)
      .then(r => r.ok ? r.text() : null)
      .then(svg => {
        if (!svg) return;

        document.querySelectorAll(`i.${iconClass}`).forEach(iEl => {

          const span = document.createElement("span");
          span.innerHTML = svg;

          const svgEl = span.querySelector("svg");
          if (!svgEl) return;

          // Style par défaut FA-like
          svgEl.style.width = "1em";
          svgEl.style.height = "1em";
          svgEl.style.verticalAlign = "middle";
          svgEl.style.fill = "currentColor";

          // Garde les classes custom
          iEl.classList.forEach(c => {
            if (!c.startsWith("fa")) svgEl.classList.add(c);
          });

          iEl.replaceWith(svgEl);
        });
      });
  });
});
