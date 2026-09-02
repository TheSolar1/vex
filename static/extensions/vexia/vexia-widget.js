// ══════════════════════════════════════════════════════════════════
// vexia-widget.js — bulle de chat flottante VexIA, a inclure sur
// n'importe quelle page VEX (fchier, sitec, ...) via :
//   <script src="/static/extensions/vexia/vexia-widget.js"></script>
//
// Un appel a /status est fait au chargement pour savoir si la bulle
// doit meme etre injectee (extension configuree + reglage personnel
// "bulle flottante" active, voir Mon Compte > Affichage personnalisé).
// N'affiche rien si l'utilisateur n'a pas acces a l'extension ou a
// desactive la bulle (401/403/preference — pas de bulle qui ne mene
// nulle part, ou que l'utilisateur ne veut pas voir).
// ══════════════════════════════════════════════════════════════════
(async function(){
    if(document.getElementById('vexia-widget-bulle')) return; // deja injecte

    try{
        const r = await fetch('/api/ext/vexia/status', {credentials:'include'});
        if(!r.ok) return;
        const d = await r.json();
        if(!d.success || d.data.widget_on === false) return;
    }catch(e){ return; }

    const style = document.createElement('style');
    style.textContent = `
#vexia-widget-bulle{position:fixed;bottom:92px;right:22px;width:52px;height:52px;border-radius:50%;
    background:var(--accent,#4caf50);box-shadow:0 4px 16px rgba(0,0,0,.25);border:none;cursor:pointer;
    display:flex;align-items:center;justify-content:center;z-index:9998;transition:transform .15s}
#vexia-widget-bulle:hover{transform:scale(1.06)}
#vexia-widget-bulle img{width:24px;height:24px;filter:brightness(0) invert(1)}
#vexia-widget-panel{position:fixed;bottom:156px;right:22px;width:340px;max-width:calc(100vw - 32px);
    height:440px;max-height:calc(100vh - 120px);background:var(--surface,#fff);border:1px solid var(--border,#e2e6ec);
    border-radius:14px;box-shadow:0 8px 32px rgba(0,0,0,.3);z-index:9999;display:none;flex-direction:column;overflow:hidden}
#vexia-widget-panel.open{display:flex}
#vexia-widget-head{padding:11px 14px;background:var(--nav-gradient,linear-gradient(135deg,#4caf50,#2e7d32));
    color:#fff;font-weight:700;font-size:.86rem;display:flex;align-items:center;gap:8px}
#vexia-widget-head img{width:16px;height:16px;filter:brightness(0) invert(1)}
#vexia-widget-close{margin-left:auto;background:none;border:none;color:#fff;cursor:pointer;font-size:1.1rem;opacity:.85;line-height:1}
#vexia-widget-close:hover{opacity:1}
#vexia-widget-msgs{flex:1;overflow-y:auto;padding:12px;display:flex;flex-direction:column;gap:8px;font-size:.82rem}
.vw-msg{max-width:85%;padding:7px 11px;border-radius:10px;line-height:1.45;white-space:pre-wrap;word-break:break-word}
.vw-msg.user{align-self:flex-end;background:var(--accent,#4caf50);color:#fff;border-bottom-right-radius:3px}
.vw-msg.assistant{align-self:flex-start;background:var(--surface2,#f7f9fb);border:1px solid var(--border,#e2e6ec);
    color:var(--text,#1c1e21);border-bottom-left-radius:3px}
.vw-msg.erreur{align-self:center;background:rgba(220,38,38,.08);color:#dc2626;font-size:.78rem;text-align:center}
.vw-vide{margin:auto;text-align:center;color:var(--text-m,#9e9e9e);font-size:.78rem;padding:16px}
#vexia-widget-bar{border-top:1px solid var(--border,#e2e6ec);padding:8px;display:flex;gap:6px}
#vexia-widget-input{flex:1;resize:none;max-height:70px;padding:7px 9px;border:1px solid var(--border,#e2e6ec);
    border-radius:8px;font-size:.82rem;font-family:inherit;background:var(--input,#fff);color:var(--text,#1c1e21)}
#vexia-widget-input:focus{outline:none;border-color:var(--accent,#4caf50)}
#vexia-widget-send{border:none;border-radius:8px;background:var(--accent,#4caf50);color:#fff;width:36px;flex-shrink:0;cursor:pointer}
#vexia-widget-send:disabled{opacity:.5;cursor:not-allowed}
.vw-pending{align-self:stretch;border:1px solid var(--border,#e2e6ec);border-radius:10px;padding:9px 11px;font-size:.8rem;background:var(--surface2,#f7f9fb)}
.vw-pending.admin{border-color:#dc2626;background:rgba(220,38,38,.06)}
.vw-pending-badge{display:inline-block;font-size:.65rem;font-weight:700;text-transform:uppercase;letter-spacing:.04em;
    padding:1px 6px;border-radius:4px;background:#dc2626;color:#fff;margin-bottom:5px}
.vw-pending-label{margin-bottom:8px;line-height:1.4}
.vw-pending-actions{display:flex;gap:8px}
.vw-pending-actions button{flex:1;border:none;border-radius:7px;padding:6px 0;font-size:.78rem;cursor:pointer;font-weight:600}
.vw-pending-confirm{background:var(--accent,#4caf50);color:#fff}
.vw-pending-cancel{background:var(--surface,#fff);border:1px solid var(--border,#e2e6ec) !important;color:var(--text,#1c1e21)}
.vw-pending-actions button:disabled{opacity:.5;cursor:not-allowed}
`;
    document.head.appendChild(style);

    const bulle = document.createElement('button');
    bulle.id = 'vexia-widget-bulle';
    bulle.title = 'VexIA — assistant IA';
    bulle.innerHTML = '<img src="/static/img/solid/robot.svg" alt="">';

    const panel = document.createElement('div');
    panel.id = 'vexia-widget-panel';
    panel.innerHTML = `
        <div id="vexia-widget-head"><img src="/static/img/solid/robot.svg" alt=""> VexIA
            <button id="vexia-widget-close" title="Fermer">✕</button></div>
        <div id="vexia-widget-msgs"><div class="vw-vide">Posez une question a VexIA.</div></div>
        <div id="vexia-widget-bar">
            <textarea id="vexia-widget-input" rows="1" placeholder="Message…"></textarea>
            <button id="vexia-widget-send" title="Envoyer">➤</button>
        </div>`;

    document.body.appendChild(bulle);
    document.body.appendChild(panel);

    let historique = [];
    let enCours = false;
    let statutVerifie = false;

    function ajouter(role, texte){
        const box = document.getElementById('vexia-widget-msgs');
        const vide = box.querySelector('.vw-vide');
        if(vide) vide.remove();
        const div = document.createElement('div');
        div.className = 'vw-msg ' + role;
        div.textContent = texte;
        box.appendChild(div);
        box.scrollTop = box.scrollHeight;
        return div;
    }

    async function verifierAcces(){
        if(statutVerifie) return true;
        try{
            const r = await fetch('/api/ext/vexia/status', {credentials:'include'});
            if(!r.ok){ bulle.remove(); panel.remove(); return false; }
            statutVerifie = true;
            const d = await r.json();
            if(d.success && d.data && !d.data.configure){
                ajouter('erreur', "VexIA n'est pas configure (cle API manquante cote admin).");
            }
            return true;
        }catch(e){ return true; }
    }

    function ajouterPending(action){
        const box = document.getElementById('vexia-widget-msgs');
        const vide = box.querySelector('.vw-vide');
        if(vide) vide.remove();
        const div = document.createElement('div');
        div.className = 'vw-pending' + (action.tier === 'admin' ? ' admin' : '');
        div.innerHTML = (action.tier === 'admin' ? '<div class="vw-pending-badge">Action admin</div>' : '')
            + '<div class="vw-pending-label"></div>'
            + '<div class="vw-pending-actions">'
            + '<button class="vw-pending-cancel" type="button">Annuler</button>'
            + '<button class="vw-pending-confirm" type="button">Confirmer</button>'
            + '</div>';
        div.querySelector('.vw-pending-label').textContent = action.label;
        const btnOk = div.querySelector('.vw-pending-confirm');
        const btnNo = div.querySelector('.vw-pending-cancel');
        const resoudre = async (decision) => {
            btnOk.disabled = true; btnNo.disabled = true;
            try{
                const r = await fetch('/api/ext/vexia/confirm', {
                    method:'POST',
                    headers:{'Content-Type':'application/json'},
                    credentials:'include',
                    body: JSON.stringify({action_id: action.id, decision}),
                });
                const d = await r.json();
                div.remove();
                if(decision === 'cancel'){
                    ajouter('assistant', 'Action annulée.');
                    return;
                }
                if(d.success){
                    ajouter('assistant', d.reply || 'Action effectuée.');
                    if(d.reply){
                        historique.push({role:'assistant', content: d.reply});
                    }
                }else{
                    ajouter('erreur', d.error || 'Erreur inconnue.');
                }
            }catch(e){
                div.remove();
                ajouter('erreur', 'Erreur réseau.');
            }
        };
        btnOk.addEventListener('click', () => resoudre('confirm'));
        btnNo.addEventListener('click', () => resoudre('cancel'));
        box.appendChild(div);
        box.scrollTop = box.scrollHeight;
    }

    async function envoyer(){
        if(enCours) return;
        const zone = document.getElementById('vexia-widget-input');
        const message = zone.value.trim();
        if(!message) return;
        ajouter('user', message);
        zone.value = '';
        enCours = true;
        document.getElementById('vexia-widget-send').disabled = true;
        try{
            const r = await fetch('/api/ext/vexia/chat', {
                method:'POST',
                headers:{'Content-Type':'application/json'},
                credentials:'include',
                body: JSON.stringify({message, history: historique}),
            });
            const d = await r.json();
            if(d.success){
                if(d.reply){
                    ajouter('assistant', d.reply);
                    historique.push({role:'user', content: message});
                    historique.push({role:'assistant', content: d.reply});
                }else{
                    historique.push({role:'user', content: message});
                }
                if(d.pending_action){
                    ajouterPending(d.pending_action);
                }
            }else{
                ajouter('erreur', d.error || 'Erreur inconnue.');
            }
        }catch(e){
            ajouter('erreur', 'Erreur reseau.');
        }
        enCours = false;
        document.getElementById('vexia-widget-send').disabled = false;
    }

    bulle.addEventListener('click', async ()=>{
        const ok = await verifierAcces();
        if(!ok) return;
        panel.classList.toggle('open');
        if(panel.classList.contains('open')) document.getElementById('vexia-widget-input').focus();
    });
    document.getElementById('vexia-widget-close').addEventListener('click', ()=> panel.classList.remove('open'));
    document.getElementById('vexia-widget-send').addEventListener('click', envoyer);
    document.getElementById('vexia-widget-input').addEventListener('keydown', e=>{
        if(e.key === 'Enter' && !e.shiftKey){ e.preventDefault(); envoyer(); }
    });
})();
