<?php
require_once __DIR__ . '/config.php';

// Lister les documents
$documents = [];
if (is_dir(DOCUMENTS_DIR)) {
    $files = array_diff(scandir(DOCUMENTS_DIR), ['.', '..']);
    foreach ($files as $file) {
        if (!is_dir(DOCUMENTS_DIR . $file)) {
            $documents[] = $file;
        }
    }
}
?>
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Mes Documents ONLYOFFICE</title>
    <style>
        * { box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
        }
        .container {
            max-width: 900px;
            margin: 0 auto;
        }
        h1 {
            color: white;
            text-align: center;
            margin-bottom: 30px;
            font-size: 2.5em;
            text-shadow: 2px 2px 4px rgba(0,0,0,0.2);
        }
        .card {
            background: white;
            border-radius: 12px;
            padding: 25px;
            margin-bottom: 20px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.2);
        }
        h2 {
            margin-top: 0;
            color: #333;
            font-size: 1.5em;
        }
        .form-group {
            display: flex;
            gap: 10px;
            margin-top: 15px;
        }
        input[type="text"] {
            flex: 1;
            padding: 12px;
            border: 2px solid #e0e0e0;
            border-radius: 6px;
            font-size: 16px;
        }
        input[type="text"]:focus {
            outline: none;
            border-color: #667eea;
        }
        .btn {
            background: #667eea;
            color: white;
            border: none;
            padding: 12px 24px;
            border-radius: 6px;
            font-size: 16px;
            cursor: pointer;
            text-decoration: none;
            display: inline-block;
            transition: background 0.3s;
        }
        .btn:hover {
            background: #5568d3;
        }
        .document-list {
            list-style: none;
            padding: 0;
            margin: 15px 0 0 0;
        }
        .document-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 15px;
            border-bottom: 1px solid #f0f0f0;
            transition: background 0.2s;
        }
        .document-item:hover {
            background: #f9f9f9;
        }
        .document-item:last-child {
            border-bottom: none;
        }
        .document-name {
            display: flex;
            align-items: center;
            gap: 10px;
            color: #333;
            font-size: 16px;
        }
        .icon {
            font-size: 24px;
        }
        .empty {
            text-align: center;
            padding: 40px;
            color: #999;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>📄 ONLYOFFICE Documents</h1>
        
        <div class="card">
            <h2>✨ Créer un nouveau document</h2>
            <form method="GET" action="editor.php" class="form-group">
                <input type="text" name="file" placeholder="nom-du-document.docx" required>
                <button type="submit" class="btn">Créer</button>
            </form>
        </div>
        
        <div class="card">
            <h2>📁 Documents existants</h2>
            <?php if (empty($documents)): ?>
                <div class="empty">
                    <p>Aucun document pour le moment.</p>
                    <p>Créez votre premier document ci-dessus !</p>
                </div>
            <?php else: ?>
                <ul class="document-list">
                    <?php foreach ($documents as $doc): ?>
                        <li class="document-item">
                            <span class="document-name">
                                <span class="icon">📄</span>
                                <?php echo htmlspecialchars($doc); ?>
                            </span>
                            <a href="editor.php?file=<?php echo urlencode($doc); ?>" class="btn">Ouvrir</a>
                        </li>
                    <?php endforeach; ?>
                </ul>
            <?php endif; ?>
        </div>
    </div>
</body>
</html>
