// ══════════════════════════════════════════════════════════════════
// notice_cloudsync.rs — notice "comment utiliser" incluse dans le zip de
// vex-cloudsync.exe (voir appareil.rs::telecharger_bundle), traduite dans
// les langues deja supportees par le site (voir function.rs::SUPPORTED_LANGS)
// -- le zip est genere a la volee par utilisateur, dans SA langue de compte
// (function::get_user_language), pas la langue systeme de la machine qui
// telecharge.
// ══════════════════════════════════════════════════════════════════

/// Retourne (nom_de_fichier, contenu) pour la langue donnee. Retombe sur le
/// francais si la langue n'est pas reconnue (ne devrait pas arriver :
/// get_user_language() ne renvoie que des codes de SUPPORTED_LANGS).
pub fn contenu(langue: &str) -> (&'static str, &'static str) {
    match langue {
        "en" => ("readme.txt", EN),
        "es" => ("readme.txt", ES),
        "de" => ("readme.txt", DE),
        "it" => ("readme.txt", IT),
        "pt" => ("readme.txt", PT),
        "ru" => ("readme.txt", RU),
        "zh" => ("readme.txt", ZH),
        "ja" => ("readme.txt", JA),
        "ar" => ("readme.txt", AR),
        _ => ("lisez-moi.txt", FR),
    }
}

const FR: &str = r#"VEX Cloud Sync — Comment l'utiliser
====================================

1. Lance vex-cloudsync.exe (double-clic).

2. Windows peut afficher un avertissement SmartScreen ("Windows a protege
   votre PC") au premier lancement -- clique "Informations complementaires"
   puis "Executer quand meme". C'est normal pour un logiciel encore peu
   diffuse, meme signe numeriquement.

3. Une fenetre de connexion s'affiche : indique l'adresse de ton serveur VEX
   (ou laisse le champ par defaut) ainsi que ton mot de passe de compte.

4. Une fois connecte, l'application tourne en arriere-plan -- pas de fenetre
   de commande. Une icone apparait dans la barre des taches, pres de
   l'horloge.

5. Un dossier synchronise apparait dans l'Explorateur Windows, avec le
   statut de synchronisation visible directement dans la colonne "Statut".

Pour quitter l'application : clic droit sur l'icone dans la barre des
taches.

IMPORTANT
---------
- Version encore en developpement : n'utilise pas ce dossier pour des
  donnees importantes pour le moment.
- Necessite Windows 10 version 1709 (Fall Creators Update) ou plus recent.
"#;

const EN: &str = r#"VEX Cloud Sync — How to use
============================

1. Launch vex-cloudsync.exe (double-click).

2. Windows may show a SmartScreen warning ("Windows protected your PC") on
   first launch -- click "More info" then "Run anyway". This is normal for
   software that is not yet widely distributed, even when digitally signed.

3. A login window appears: enter your VEX server address (or leave the
   default) and your account password.

4. Once connected, the app runs in the background -- no console window. An
   icon appears in the system tray, near the clock.

5. A synced folder appears in Windows Explorer, with the sync status shown
   directly in the "Status" column.

To quit the app: right-click the tray icon.

IMPORTANT
---------
- Still under development: do not use this folder for important data yet.
- Requires Windows 10 version 1709 (Fall Creators Update) or later.
"#;

const ES: &str = r#"VEX Cloud Sync — Como usar
============================

1. Inicia vex-cloudsync.exe (doble clic).

2. Puede que Windows muestre un aviso de SmartScreen ("Windows protegio tu
   PC") en el primer inicio -- haz clic en "Mas informacion" y luego en
   "Ejecutar de todos modos". Es normal para software poco distribuido,
   incluso firmado digitalmente.

3. Aparece una ventana de inicio de sesion: introduce la direccion de tu
   servidor VEX (o deja la de por defecto) y la contrasena de tu cuenta.

4. Una vez conectado, la aplicacion se ejecuta en segundo plano -- sin
   ventana de consola. Aparece un icono en la bandeja del sistema, junto
   al reloj.

5. Aparece una carpeta sincronizada en el Explorador de Windows, con el
   estado de sincronizacion visible en la columna "Estado".

Para salir: clic derecho en el icono de la bandeja.

IMPORTANTE
----------
- Version aun en desarrollo: no uses esta carpeta para datos importantes
  por ahora.
- Requiere Windows 10 version 1709 (Fall Creators Update) o posterior.
"#;

const DE: &str = r#"VEX Cloud Sync — Verwendung
============================

1. Starte vex-cloudsync.exe (Doppelklick).

2. Windows zeigt beim ersten Start moeglicherweise eine SmartScreen-Warnung
   ("Windows hat Ihren PC geschuetzt") -- klicke auf "Weitere Informationen"
   und dann auf "Trotzdem ausfuehren". Das ist normal bei noch wenig
   verbreiteter Software, auch wenn sie digital signiert ist.

3. Ein Anmeldefenster erscheint: gib die Adresse deines VEX-Servers ein
   (oder lass das Standardfeld) sowie dein Konto-Passwort.

4. Nach der Verbindung laeuft die App im Hintergrund -- kein Konsolenfenster.
   Ein Symbol erscheint in der Taskleiste, neben der Uhr.

5. Ein synchronisierter Ordner erscheint im Windows-Explorer, mit dem
   Synchronisierungsstatus direkt in der Spalte "Status".

Zum Beenden: Rechtsklick auf das Taskleistensymbol.

WICHTIG
-------
- Noch in Entwicklung: verwende diesen Ordner vorerst nicht fuer wichtige
  Daten.
- Erfordert Windows 10 Version 1709 (Fall Creators Update) oder neuer.
"#;

const IT: &str = r#"VEX Cloud Sync — Come usare
============================

1. Avvia vex-cloudsync.exe (doppio clic).

2. Windows potrebbe mostrare un avviso SmartScreen ("Windows ha protetto il
   PC") al primo avvio -- clicca su "Altre informazioni" poi su "Esegui
   comunque". E normale per software poco diffuso, anche se firmato
   digitalmente.

3. Appare una finestra di accesso: inserisci l'indirizzo del tuo server VEX
   (o lascia quello predefinito) e la password del tuo account.

4. Una volta connesso, l'app funziona in background -- nessuna finestra
   console. Un'icona appare nella barra delle applicazioni, vicino
   all'orologio.

5. Una cartella sincronizzata appare in Esplora risorse, con lo stato di
   sincronizzazione visibile nella colonna "Stato".

Per uscire: clic destro sull'icona nella barra delle applicazioni.

IMPORTANTE
----------
- Versione ancora in sviluppo: non usare questa cartella per dati
  importanti per ora.
- Richiede Windows 10 versione 1709 (Fall Creators Update) o successiva.
"#;

const PT: &str = r#"VEX Cloud Sync — Como usar
============================

1. Inicie o vex-cloudsync.exe (clique duplo).

2. O Windows pode mostrar um aviso do SmartScreen ("O Windows protegeu o
   seu PC") no primeiro arranque -- clique em "Mais informacoes" e depois
   em "Executar mesmo assim". Isto e normal para software ainda pouco
   distribuido, mesmo assinado digitalmente.

3. Aparece uma janela de login: indique o endereco do seu servidor VEX (ou
   deixe o valor padrao) e a palavra-passe da sua conta.

4. Depois de ligado, a aplicacao corre em segundo plano -- sem janela de
   consola. Aparece um icone na barra de tarefas, perto do relogio.

5. Uma pasta sincronizada aparece no Explorador do Windows, com o estado
   de sincronizacao visivel na coluna "Estado".

Para sair: clique com o botao direito no icone da barra de tarefas.

IMPORTANTE
----------
- Versao ainda em desenvolvimento: nao uses esta pasta para dados
  importantes por agora.
- Requer Windows 10 versao 1709 (Fall Creators Update) ou posterior.
"#;

const RU: &str = r#"VEX Cloud Sync — Как использовать
====================================

1. Запустите vex-cloudsync.exe (двойной клик).

2. При первом запуске Windows может показать предупреждение SmartScreen
   ("Windows защитил ваш компьютер") -- нажмите "Дополнительно", затем
   "Выполнить в любом случае". Это нормально для малораспространённого
   ПО, даже подписанного.

3. Появится окно входа: укажите адрес вашего сервера VEX (или оставьте
   значение по умолчанию) и пароль от аккаунта.

4. После подключения приложение работает в фоне -- без окна консоли.
   В области уведомлений (рядом с часами) появится значок.

5. В проводнике Windows появится синхронизированная папка, статус
   синхронизации виден прямо в столбце "Статус".

Чтобы выйти: правый клик по значку в области уведомлений.

ВАЖНО
-----
- Версия ещё в разработке: пока не используйте эту папку для важных
  данных.
- Требуется Windows 10 версии 1709 (Fall Creators Update) или новее.
"#;

const ZH: &str = r#"VEX Cloud Sync — 使用说明
============================

1. 启动 vex-cloudsync.exe（双击）。

2. 首次启动时 Windows 可能会显示 SmartScreen 警告（"Windows 已保护你的电脑"）——
   点击"更多信息"，再点击"仍要运行"。对于分发量较小的软件，即使已数字签名，
   出现此提示也是正常的。

3. 会出现登录窗口：输入你的 VEX 服务器地址（或保留默认值）以及账户密码。

4. 连接成功后，应用会在后台运行——没有控制台窗口。系统托盘（时钟旁）会
   出现一个图标。

5. Windows 资源管理器中会出现一个同步文件夹，同步状态会直接显示在
   "状态"列中。

退出应用：右键点击托盘图标。

重要提示
--------
- 该版本仍在开发中：请暂时不要用此文件夹存放重要数据。
- 需要 Windows 10 版本 1709（秋季创意者更新）或更高版本。
"#;

const JA: &str = r#"VEX Cloud Sync — 使い方
============================

1. vex-cloudsync.exe を起動します（ダブルクリック）。

2. 初回起動時に Windows の SmartScreen 警告（「Windows によって PC が保護
   されました」）が表示されることがあります。「詳細情報」をクリックし、
   続けて「実行」をクリックしてください。まだ広く配布されていない
   ソフトウェアでは、デジタル署名されていてもこの表示は正常です。

3. ログイン画面が表示されます。VEX サーバーのアドレス（またはデフォルトの
   ままでも可）とアカウントのパスワードを入力してください。

4. 接続後、アプリはバックグラウンドで動作します（コンソール画面はあり
   ません）。タスクトレイ（時計の近く）にアイコンが表示されます。

5. エクスプローラーに同期フォルダが表示され、「状態」列に同期状況が
   直接表示されます。

終了するには：タスクトレイのアイコンを右クリックしてください。

重要
----
- まだ開発中のバージョンです。現時点では重要なデータをこのフォルダに
  置かないでください。
- Windows 10 バージョン 1709（Fall Creators Update）以降が必要です。
"#;

const AR: &str = r#"VEX Cloud Sync — طريقة الاستخدام
====================================

1. شغّل vex-cloudsync.exe (نقرة مزدوجة).

2. قد تظهر رسالة تحذير SmartScreen من ويندوز ("قام Windows بحماية جهاز
   الكمبيوتر") عند التشغيل الأول -- اضغط على "معلومات إضافية" ثم على
   "تشغيل على أي حال". هذا أمر طبيعي لبرنامج غير منتشر بعد، حتى لو كان
   موقّعًا رقميًا.

3. ستظهر نافذة تسجيل الدخول: أدخل عنوان خادم VEX الخاص بك (أو اترك العنوان
   الافتراضي) وكلمة مرور حسابك.

4. بعد الاتصال، يعمل التطبيق في الخلفية -- بدون نافذة طرفية. سيظهر رمز في
   شريط المهام، بجانب الساعة.

5. سيظهر مجلد متزامن في مستكشف ويندوز، مع عرض حالة المزامنة مباشرة في عمود
   "الحالة".

للخروج: انقر بزر الماوس الأيمن على الرمز في شريط المهام.

مهم
---
- النسخة ما زالت قيد التطوير: لا تستخدم هذا المجلد لبيانات مهمة حاليًا.
- يتطلب Windows 10 الإصدار 1709 (Fall Creators Update) أو أحدث.
"#;
