--!strict
-- Projeto do FilmCraft da pasta inteira do item: vídeos em ordem, só com a fala (J/L entre as falas),
-- legenda presa à fala, fotos da pasta como b-roll e a identidade da marca.
local ng3 = require("@ng3")

local M = {}

function M.info()
	return {
		id = "montar-pasta",
		nome = "Montar projeto da pasta",
		descricao = "Junta todos os vídeos e fotos da pasta num .fcproj: transcreve o que falta, tira o silêncio "
			.. "de quem fala (vídeo sem fala entra inteiro e sem legenda), põe a legenda e as fotos por cima "
			.. "do vídeo a partir de quando alguém fala em \"foto\". O projeto anterior vai para projetos_antigos/.",
		aplicaA = { "post", "corte" },
		params = {
			{ id = "cortar", rotulo = "Tirar o silêncio", tipo = "booleano", padrao = true },
			{ id = "preset", rotulo = "Ritmo dos cortes", tipo = "escolha", opcoes = { "podcast", "padrao", "agressivo" },
				padrao = "podcast", ajuda = "podcast = pausas longas e J/L · agressivo = ritmo de reels" },
			{ id = "fotos", rotulo = "Fotos como b-roll", tipo = "booleano", padrao = true },
			{ id = "foto_s", rotulo = "Segundos por foto", tipo = "numero", padrao = 3, min = 1, max = 8, passo = 0.5 },
		},
	}
end

function M.rodar(itens, p)
	local n = 0
	for _, item in itens do
		local pasta = if item.tipo == "corte" then item.arquivo else item.pasta
		if pasta == nil or pasta == "" then
			continue
		end
		local cmd = { "python3", "src/projetos/pasta.py", pasta, "--aplicar", "--preset", p.preset,
			"--foto-s", string.format("%.2f", p.foto_s) }
		if not p.cortar then
			table.insert(cmd, "--sem-cortar")
		end
		if not p.fotos then
			table.insert(cmd, "--sem-fotos")
		end
		ng3.processo(cmd)
		n += 1
	end
	ng3.avisar(`{n} pasta(s) na fila`)
end

return M
