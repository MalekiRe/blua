local app = ...

local i = 0.0;

function my_system(query)
    return
    --[[i = i + 1.0
    local last = nil
    for transform in query:iter() do
        last = transform.translation
        transform.translation.x = math.sin(i)
    end]]
end

function my_system2(commands, query)
    local awa = 0
    for transform in query:iter() do
        awa = awa + 1
        print(transform)
    end
    if (awa == 0) then
        commands:spawn({ bevy_transform.components.transform.Transform.default() })
    end
end

app:register_system(my_system2,
{
  Commands,
  {bevy_transform.components.transform.Transform.mut}
})